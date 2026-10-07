use super::*;

pub(crate) fn bind_sql_keys(cx: &mut App) {
    let key = if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    };
    // The descendant binding overrides Input's secondary-Enter newline action.
    cx.bind_keys([
        KeyBinding::new(key, RunQuery, Some("SQLQuery")),
        KeyBinding::new(key, RunQuery, Some("SQLQuery > Input")),
    ]);
}

pub(super) fn run_source(
    source: Arc<Source>,
    sql: String,
    profile: String,
    cancel: &Cancellation,
    progress: &AtomicUsize,
) -> anyhow::Result<Arc<parquetviewer::query::QueryResult>> {
    #[cfg(all(test, feature = "ui-tests"))]
    {
        let executable = std::env::current_exe()?;
        let executable = executable
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Missing test build directory"))?
            .join(format!("parquetviewer{}", std::env::consts::EXE_SUFFIX));
        parquetviewer::query::run_with_executable(
            source,
            sql,
            profile,
            cancel,
            progress,
            &executable,
            data::all_rows_budget().min(1 << 30),
        )
    }
    #[cfg(not(all(test, feature = "ui-tests")))]
    parquetviewer::query::run(source, sql, profile, cancel, progress)
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::{Filter, FilterOp, OpenQuery, Panel, RowLimit, Settings, SettingsStore, Viewer};
    use gpui_kit::{
        AppContext, Bounds, ScrollDelta, TestAppContext, WindowBounds, WindowOptions, point, px,
        size, test::TestWindowExt,
    };
    use parquetviewer::data;
    use std::sync::Arc;

    #[gpui_kit::test]
    fn long_sql_errors_leave_the_editor_usable_and_details_scrollable(cx: &mut TestAppContext) {
        let fixture = super::super::tests::fixture(1200);
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        store
            .save(&Settings {
                association_prompted: true,
                ..Default::default()
            })
            .unwrap();
        cx.update(gpui_omarchy::init);
        cx.update(super::bind_sql_keys);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(1000.0), px(500.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| Viewer::new(Some(fixture.path().to_owned()), store, window, cx))
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |view, cx| {
                view.open_query(&OpenQuery, window, cx);
                view.sql_input.update(cx, |input, cx| {
                    input.set_value("SELECT absent FROM data", window, cx)
                });
                view.run_query(window, cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
        let diagnostic = content.update(cx, |view, cx| {
            assert!(matches!(view.panel, Some(Panel::Sql)));
            assert_eq!(view.page.as_ref().unwrap().frame.height(), 500);
            let diagnostic = format!(
                "{}\n{}",
                view.error.as_ref().unwrap(),
                format!("Resolved plan: {}\n", "wide_column, ".repeat(100)).repeat(200)
            );
            view.error = Some(diagnostic.clone());
            cx.notify();
            diagnostic
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("error-banner").bounds().size.height < px(100.0));
            assert!(window.find("sql-editor").bounds().size.height >= px(160.0));
            window.click("error-details-toggle", cx);
            window.render_frame(cx);
            assert!(window.find("error-diagnostic").bounds().size.height <= px(60.0));
            assert!(window.find("sql-editor").bounds().size.height >= px(100.0));
            assert!(window.find("run-sql").visible());
            window.scroll("error-diagnostic", ScrollDelta::Lines(point(0.0, -3.0)), cx);
            window.render_frame(cx);
            assert!(content.read(cx).error_scroll.offset().y < px(0.0));
            window.click("error-copy", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().unwrap(),
                diagnostic
            );
            window.click("close-panel", cx);
            window.render_frame(cx);
            assert!(window.find("error-banner").bounds().size.height < px(160.0));
            assert!(window.find("table-frame").bounds().size.height > px(200.0));
            content.update(cx, |view, cx| view.open_query(&OpenQuery, window, cx));
            window.render_frame(cx);
            window.click("sql-editor", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            window.input("SELECT COUNT(*) AS n FROM data", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-enter"
                } else {
                    "ctrl-enter"
                },
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        content.read_with(cx, |view, _| {
            assert!(view.error.is_none(), "{:?}", view.error);
            assert!(view.panel.is_none());
            assert_eq!(view.page.as_ref().unwrap().cell(0, 0).0, "1200");
        });
    }

    #[gpui_kit::test]
    fn failed_saved_sql_opens_the_normal_source_and_keeps_the_draft(cx: &mut TestAppContext) {
        use parquetviewer::{
            settings::{FileView, SqlView},
            source::Location,
        };
        let fixture = super::super::tests::fixture(1200);
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        let mut settings = Settings {
            row_limit: RowLimit::All,
            ..Default::default()
        };
        settings.remember(FileView {
            path: fixture.path().to_owned(),
            location: Some(Location::local(fixture.path().to_owned())),
            offset: 500,
            filters: vec![Filter {
                column: "removed_column".into(),
                op: FilterOp::IsNull,
                value: String::new(),
                upper: String::new(),
            }],
            sql: SqlView {
                query: Some("SELECT removed_column FROM data".into()),
                text: "SELECT COUNT(*) AS rows FROM data".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        store.save(&settings).unwrap();
        cx.update(gpui_omarchy::init);
        let (_, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(fixture.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        content.read_with(cx, |view, _| {
            assert!(view.sql.is_none());
            assert!(view.error.is_none(), "{:?}", view.error);
            assert!(
                view.notice
                    .as_ref()
                    .unwrap()
                    .contains("Saved SQL could not be restored")
            );
            assert_eq!(view.page.as_ref().unwrap().frame.height(), 500);
            assert_eq!(view.page.as_ref().unwrap().offset, 500);
            assert!(view.filters.is_empty());
            assert_eq!(view.sql_text, "SELECT COUNT(*) AS rows FROM data");
        });
        assert!(store.load().unwrap().files[0].sql.query.is_none());
    }

    #[gpui_kit::test]
    fn run_shortcut_works_with_the_multiline_editor_focused(cx: &mut TestAppContext) {
        let fixture = super::super::tests::fixture(1200);
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        cx.update(super::bind_sql_keys);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(fixture.path().to_owned()), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |view, cx| view.open_query(&OpenQuery, window, cx));
            window.render_frame(cx);
            let input = content.read(cx).sql_input.clone();
            window.click("sql-editor", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            window.input("SELECT COUNT(*) AS n", cx);
            window.press("enter", cx);
            window.input("FROM data", cx);
            assert_eq!(
                input.read(cx).value().as_str(),
                "SELECT COUNT(*) AS n\nFROM data"
            );
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-enter"
                } else {
                    "ctrl-enter"
                },
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        content.read_with(cx, |view, _| {
            assert!(view.sql.is_some(), "{:?}", view.error);
            assert_eq!(view.sql_text, "SELECT COUNT(*) AS n\nFROM data");
            assert_eq!(view.page.as_ref().unwrap().cell(0, 0).0, "1200");
        });
    }

    #[gpui_kit::test]
    fn opening_a_folder_never_restores_all_implicitly(cx: &mut TestAppContext) {
        let fixture = super::super::tests::fixture(1200);
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("dataset");
        std::fs::create_dir_all(root.join("year=2026")).unwrap();
        std::fs::copy(fixture.path(), root.join("year=2026/data.parquet")).unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        let settings = Settings {
            row_limit: RowLimit::All,
            ..Default::default()
        };
        store.save(&settings).unwrap();
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(root.clone()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        content.read_with(cx, |view, _| {
            assert!(view.page.as_ref().unwrap().source.is_dataset());
            assert_eq!(
                view.page.as_ref().unwrap().frame.height(),
                data::DEFAULT_PAGE_SIZE
            );
            assert_eq!(view.settings.row_limit, RowLimit::default());
        });
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |view, cx| view.set_row_limit(RowLimit::All, window, cx))
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            content.read_with(cx, |v, _| v.page.as_ref().unwrap().frame.height()),
            1200
        );
        assert_eq!(store.load().unwrap().row_limit, RowLimit::All);
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        drop(content);
        cx.run_until_parked();
        let (_, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(root), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        assert_eq!(
            content.read_with(cx, |v, _| v.page.as_ref().unwrap().frame.height()),
            data::DEFAULT_PAGE_SIZE
        );
    }

    #[gpui_kit::test]
    fn sql_owns_order_and_filters_restores_saved_results_and_exits_to_the_prior_view(
        cx: &mut TestAppContext,
    ) {
        let fixture = super::super::tests::fixture(1200);
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        let settings = Settings {
            row_limit: RowLimit::Paged(100),
            ..Default::default()
        };
        store.save(&settings).unwrap();
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(fixture.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        let filters = vec![Filter {
            column: "column_0".into(),
            op: FilterOp::Range,
            value: "100".into(),
            upper: "599".into(),
        }];
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| v.run_filters(filters.clone(), window, cx))
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| {
                v.sort_column(0, window, cx);
                v.navigate(3, window, cx);
            })
        })
        .unwrap();
        cx.run_until_parked();
        let sql = "SELECT column_0 AS value FROM data WHERE column_0 >= 100 ORDER BY column_0 DESC";
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| {
                v.open_query(&OpenQuery, window, cx);
                v.sql_input.update(cx, |i, cx| i.set_value(sql, window, cx));
            });
            window.render_frame(cx);
            assert!(window.find("sql-editor").visible());
            window.click("run-sql", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let cache = content.read_with(cx, |v, _| {
            assert!(v.error.is_none(), "{:?}", v.error);
            assert_eq!(v.sort, None);
            assert_eq!(v.filters, filters);
            assert_eq!(v.page.as_ref().unwrap().total_rows, 1100);
            assert_eq!(v.page.as_ref().unwrap().profiles[0].name, "value");
            assert_eq!(
                v.page
                    .as_ref()
                    .unwrap()
                    .frame
                    .column("value")
                    .unwrap()
                    .get(0)
                    .unwrap(),
                polars_core::prelude::AnyValue::Int64(1199)
            );
            v.sql.as_ref().unwrap().clone()
        });
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| {
                v.sort_column(0, window, cx);
                assert_eq!(v.sort, None);
                v.open_filter(0, window, cx);
                assert!(v.panel.is_none());
                v.run_filters(Vec::new(), window, cx);
                assert_eq!(v.filters, filters);
                v.navigate(2, window, cx);
            })
        })
        .unwrap();
        cx.run_until_parked();
        content.read_with(cx, |v, _| {
            assert!(Arc::ptr_eq(v.sql.as_ref().unwrap(), &cache))
        });
        let saved = store.load().unwrap();
        let saved = &saved.files[0];
        assert_eq!(saved.offset, 200);
        assert_eq!(saved.sql.offset, 100);
        assert_eq!(saved.sql.query.as_deref(), Some(sql));
        assert_eq!(saved.filters, filters);
        assert!(saved.sort.is_some());
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| {
                v.sql_input.update(cx, |i, cx| {
                    i.set_value("SELECT absent FROM data", window, cx)
                });
                v.run_query(window, cx);
            })
        })
        .unwrap();
        cx.run_until_parked();
        content.read_with(cx, |v, _| {
            assert!(v.error.is_some());
            assert!(Arc::ptr_eq(v.sql.as_ref().unwrap(), &cache));
        });
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        drop(content);
        cx.run_until_parked();
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(fixture.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        content.read_with(cx, |v, _| {
            assert!(v.sql.is_some(), "{:?}", v.error);
            assert_eq!(v.page.as_ref().unwrap().offset, 100);
            assert_eq!(v.sql_text, "SELECT absent FROM data");
        });
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |v, cx| v.exit_sql(window, cx))
        })
        .unwrap();
        cx.run_until_parked();
        content.read_with(cx, |v, _| {
            assert!(v.sql.is_none());
            assert_eq!(v.page.as_ref().unwrap().matching_rows, Some(500));
            assert_eq!(v.page.as_ref().unwrap().offset, 200);
            assert_eq!(v.sort, Some((0, false)));
            assert_eq!(v.filters, filters);
        });
        assert!(store.load().unwrap().files[0].sql.query.is_none());
    }
}

impl Viewer {
    pub(super) fn run_query_action(
        &mut self,
        _: &RunQuery,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.panel, Some(Panel::Sql)) {
            self.run_query(window, cx);
        }
    }
    pub(super) fn open_folder(
        &mut self,
        _: &OpenFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Hive-partitioned Parquet folder".into()),
        });
        self.dialog_task = Some(cx.spawn_in(window, async move |this, cx| {
            let selection = selection.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match selection {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.open_path(path, window, cx);
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        this.error = Some(format!("Cannot open folder picker: {error}"))
                    }
                    Err(error) => this.error = Some(format!("Folder picker closed: {error}")),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn open_query(
        &mut self,
        _: &OpenQuery,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.page.is_none() {
            return;
        }
        self.show_panel(Panel::Sql, window, cx);
    }

    pub(super) fn run_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        let location = page.source.location.clone();
        let text = self.sql_input.read(cx).value().to_string();
        if let Err(error) = parquetviewer::query::validate(&text) {
            self.error = Some(error.to_string());
            self.error_details = false;
            self.error_scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
            return;
        }
        self.sql_text = text.clone();
        self.save_settings(cx);
        self.sql_override = Some(Some(text));
        self.load_location(location, 0, false, window, cx);
    }

    pub(super) fn exit_sql(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.sql.is_none() {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        let location = page.source.location.clone();
        self.save_settings(cx);
        let offset = self
            .settings
            .file_location(&location)
            .map_or(0, |view| view.offset);
        self.sql_override = Some(None);
        self.load_location(location, offset, false, window, cx);
    }

    pub(super) fn sql_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let columns: Vec<(String, String)> = self.sql.as_ref().map_or_else(
            || {
                self.page
                    .as_ref()
                    .unwrap()
                    .profiles
                    .iter()
                    .map(|p| (p.name.clone(), p.dtype.clone()))
                    .collect()
            },
            |sql| {
                sql.schema
                    .iter()
                    .map(|(name, dtype)| (name.to_string(), dtype.to_string()))
                    .collect()
            },
        );
        let columns = uniform_list("sql-columns", columns.len(), move |range, _, cx| {
            range
                .map(|i| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .h(px(28.0))
                        .text_xs()
                        .child(div().flex_1().truncate().child(columns[i].0.clone()))
                        .child(
                            div()
                                .text_color(cx.omarchy().accent)
                                .child(columns[i].1.clone()),
                        )
                })
                .collect()
        })
        .flex_1()
        .min_h_0();
        div().flex().flex_col().flex_1().min_h_0().gap_3()
            .child(div().text_sm().text_color(cx.omarchy().secondary).child("Query the complete source as data. SQL controls filtering and sorting while active; results are paged."))
            .child(div().flex().flex_1().min_h_0().gap_3()
                .child(gpui_omarchy::textarea("sql-editor", &self.sql_input, window, cx).flex_1().min_w_0().min_h_0().font_family("monospace"))
                .child(div().flex().flex_col().w(px(220.0)).min_h_0().gap_2().child(div().text_sm().child("data · Source columns")).child(columns)))
            .child(div().text_sm().text_color(cx.omarchy().secondary).child("One read-only SELECT or WITH query. Use double quotes for column names with spaces. Cancel keeps your current view."))
            .child(div().flex().items_center().gap_3()
                .child(control("run-sql", if cfg!(target_os = "macos") { "Run query · Cmd+Enter" } else { "Run query · Ctrl+Enter" }, cx).disabled(self.loading).on_click(cx.listener(|this, _, window, cx| this.run_query(window, cx))))
                .when(self.sql.is_some(), |d| d.child(control("stop-sql", "Exit SQL mode", cx).disabled(self.loading).on_click(cx.listener(|this, _, window, cx| this.exit_sql(window, cx))))))
    }
}
