//! Table interactions and small view controls. No editing or query language.
use super::*;
use gpui_kit::base::CheckboxState;

#[derive(Clone, Debug)]
pub(super) struct Selection {
    pub anchor: (usize, usize),
    pub head: (usize, usize),
    pub whole_rows: bool,
}
impl Selection {
    pub fn contains(&self, row: usize, column: usize) -> bool {
        (self.anchor.0.min(self.head.0)..=self.anchor.0.max(self.head.0)).contains(&row)
            && (self.whole_rows
                || (self.anchor.1.min(self.head.1)..=self.anchor.1.max(self.head.1))
                    .contains(&column))
    }
}
impl Viewer {
    pub(super) fn column_width(&self, column: usize) -> f32 {
        self.widths.get(column).copied().unwrap_or(COLUMN_WIDTH)
    }
    pub(super) fn columns_width(&self, range: Range<usize>) -> f32 {
        self.columns[range]
            .iter()
            .map(|&c| self.column_width(c))
            .sum()
    }
    pub(super) fn refresh_columns(&mut self) {
        if let Some(page) = &self.page {
            self.columns = page
                .profiles
                .iter()
                .enumerate()
                .filter(|(_, p)| !self.hidden_columns.contains(&p.name))
                .map(|(i, _)| i)
                .collect();
        }
        self.selection = None;
        self.clamp_horizontal();
    }
    fn clamp_horizontal(&mut self) {
        let max = (ROW_NUMBER_WIDTH + self.columns_width(0..self.columns.len())
            - f32::from(self.viewport_width))
        .max(0.0);
        self.viewport_x = px(f32::from(self.viewport_x).clamp(-max, 0.0));
        self.horizontal.set_offset(point(self.viewport_x, px(0.0)));
    }
    fn set_width(&mut self, column: usize, width: f32, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            self.widths.resize(page.profiles.len(), COLUMN_WIDTH);
        }
        self.widths[column] = width.clamp(120.0, 800.0);
        self.clamp_horizontal();
        cx.notify();
    }
    pub(super) fn resize_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some((column, x, width)) = self.resizing {
            if event.dragging() {
                self.set_width(column, width + f32::from(event.position.x) - x, cx);
            } else {
                self.resizing = None;
                self.save_settings(cx);
            }
        }
    }
    pub(super) fn fit_column(&mut self, column: usize, cx: &mut Context<Self>) {
        let Some(page) = &self.page else {
            return;
        };
        let start = self.row_position().min(self.rows.len());
        let end = (start + 40).min(self.rows.len());
        let length = self.rows[start..end]
            .iter()
            .map(|&row| page.cell(row, column).0.chars().count())
            .chain([
                page.profiles[column].name.chars().count() + 6,
                page.profiles[column].dtype.chars().count(),
            ])
            .max()
            .unwrap_or(12);
        self.set_width(
            column,
            (length as f32 * 8.0 + 32.0).max(if self.settings.show_plots {
                220.0
            } else {
                120.0
            }),
            cx,
        );
        self.save_settings(cx);
    }
    pub(super) fn toggle_column(&mut self, column: usize, cx: &mut Context<Self>) {
        let Some(page) = &self.page else {
            return;
        };
        let name = page.profiles[column].name.clone();
        if self.hidden_columns.contains(&name) {
            self.hidden_columns.retain(|s| s != &name);
        } else if self.columns.len() > 1 {
            self.hidden_columns.push(name);
        } else {
            self.notice = Some("Keep at least one column visible".into());
        }
        self.refresh_columns();
        self.save_settings(cx);
        cx.notify();
    }
    pub(super) fn columns_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let page = self.page.as_ref().unwrap();
        let query = self.search_input.read(cx).value().trim().to_lowercase();
        let matches: Vec<_> = page
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.name.to_lowercase().contains(&query) || p.dtype.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        let owner = cx.entity();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_3()
            .child(input("visibility-search", &self.search_input, window, cx).w_full())
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(format!(
                        "{} of {} columns visible · Original file order",
                        self.columns.len(),
                        page.profiles.len()
                    ))
                    .child(
                        control("show-all-columns", "Show all", cx).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.hidden_columns.clear();
                                this.refresh_columns();
                                this.save_settings(cx);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                uniform_list("visibility-list", matches.len(), move |range, _, cx| {
                    let this = owner.read(cx);
                    let page = this.page.as_ref().unwrap();
                    range
                        .map(|row| {
                            let column = matches[row];
                            let p = &page.profiles[column];
                            let target = owner.clone();
                            gpui_omarchy::checkbox(
                                ("visible-column", column),
                                format!("{} · {}", p.name, p.dtype),
                                if this.hidden_columns.contains(&p.name) {
                                    CheckboxState::Unchecked
                                } else {
                                    CheckboxState::Checked
                                },
                                cx,
                            )
                            .h(px(40.0))
                            .on_change(move |_, _, _, cx| {
                                target.update(cx, |this, cx| this.toggle_column(column, cx))
                            })
                            .into_any_element()
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(&self.find_scroll)
                .flex_1()
                .min_h_0(),
            )
    }
    pub(super) fn select_cell(
        &mut self,
        row: usize,
        column: usize,
        extend: bool,
        whole_rows: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        let head = (
            row.min(self.rows.len() - 1),
            column.min(self.columns.len() - 1),
        );
        let anchor = if extend {
            self.selection.as_ref().map_or(head, |s| s.anchor)
        } else {
            head
        };
        self.selection = Some(Selection {
            anchor,
            head,
            whole_rows,
        });
        self.grid_focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn move_selection(
        &mut self,
        dx: isize,
        dy: isize,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading
            || self.panel.is_some()
            || self.detail.is_some()
            || self.rows.is_empty()
            || self.columns.is_empty()
        {
            return;
        }
        let previous = self
            .selection
            .as_ref()
            .map_or((self.row_position(), self.visible_columns().start), |s| {
                s.head
            });
        let row = previous
            .0
            .saturating_add_signed(dy)
            .min(self.rows.len() - 1);
        let col = previous
            .1
            .saturating_add_signed(dx)
            .min(self.columns.len() - 1);
        self.reveal_cell(row, col, extend, ScrollStrategy::Nearest, window, cx);
    }

    pub(super) fn reveal_cell(
        &mut self,
        row: usize,
        column: usize,
        extend: bool,
        strategy: ScrollStrategy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        self.select_cell(row, column, extend, false, window, cx);
        let (row, col) = self.selection.as_ref().unwrap().head;
        self.vertical.scroll_to_item_strict(row, strategy);
        let left = ROW_NUMBER_WIDTH + self.columns_width(0..col);
        let right = left + self.column_width(self.columns[col]);
        let x = -f32::from(self.viewport_x);
        if left < x {
            self.viewport_x = -px(left);
        } else if right > x + f32::from(self.viewport_width) {
            self.viewport_x = -px(right - f32::from(self.viewport_width));
        }
        self.clamp_horizontal();
        self.schedule_save(cx);
    }

    pub(super) fn table_navigation(
        &mut self,
        action: &TableNavigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading
            || self.panel.is_some()
            || self.detail.is_some()
            || self.rows.is_empty()
            || self.columns.is_empty()
        {
            return;
        }
        let (row, column) = self
            .selection
            .as_ref()
            .map_or((self.row_position(), self.visible_columns().start), |s| {
                s.head
            });
        let screen = self.vertical.0.borrow().last_item_size.map_or(1, |s| {
            ((f32::from(s.item.height) / ROW_HEIGHT).floor() as usize)
                .saturating_sub(1)
                .max(1)
        });
        use TableDestination::*;
        let (row, column, strategy) = match action.destination {
            FirstRow => (0, column, ScrollStrategy::Top),
            LastRow => (self.rows.len() - 1, column, ScrollStrategy::Bottom),
            ScreenUp | ScreenDown => {
                let top = self.row_position();
                let last = self.rows.len() - 1;
                let advance = |value: usize| {
                    if action.destination == ScreenUp {
                        value.saturating_sub(screen)
                    } else {
                        value.saturating_add(screen).min(last)
                    }
                };
                self.reveal_cell(
                    advance(row),
                    column,
                    action.extend,
                    ScrollStrategy::Nearest,
                    window,
                    cx,
                );
                // Move the viewport as well as the selection, retaining its screen position.
                self.vertical
                    .scroll_to_item_strict(advance(top), ScrollStrategy::Top);
                return;
            }
            FirstColumn => (row, 0, ScrollStrategy::Nearest),
            LastColumn => (row, self.columns.len() - 1, ScrollStrategy::Nearest),
            PreviousPage | NextPage | FirstPage | LastPage => {
                let page = self.page.as_ref().unwrap();
                let target = match action.destination {
                    PreviousPage => page.page_number().saturating_sub(1).max(1),
                    NextPage => (page.page_number() + 1).min(page.page_count()),
                    FirstPage => 1,
                    _ => page.page_count(),
                };
                let at_end = action.destination == LastPage;
                if target == page.page_number() {
                    if matches!(action.destination, FirstPage | LastPage) {
                        self.reveal_cell(
                            if at_end { self.rows.len() - 1 } else { 0 },
                            column,
                            false,
                            if at_end {
                                ScrollStrategy::Bottom
                            } else {
                                ScrollStrategy::Top
                            },
                            window,
                            cx,
                        );
                    }
                } else {
                    self.load_location_selected(
                        page.source.location.clone(),
                        (target - 1) * page.page_size,
                        false,
                        Some((at_end, column)),
                        window,
                        cx,
                    );
                }
                return;
            }
        };
        self.reveal_cell(row, column, action.extend, strategy, window, cx);
    }

    pub(super) fn select_all_cells(
        &mut self,
        _: &SelectAllCells,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading
            || self.panel.is_some()
            || self.detail.is_some()
            || self.rows.is_empty()
            || self.columns.is_empty()
        {
            return;
        }
        self.selection = Some(Selection {
            anchor: (0, 0),
            head: (self.rows.len() - 1, self.columns.len() - 1),
            whole_rows: false,
        });
        self.grid_focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn focus_page(
        &mut self,
        _: &FocusPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.panel.is_some() || self.detail.is_some() || self.page.is_none() {
            return;
        }
        self.page_input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
    }

    pub(super) fn escape_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel.is_some() || self.detail.is_some() {
            self.close_detail(window, cx);
        } else if self.selection.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    pub(super) fn show_shortcuts(
        &mut self,
        _: &ShowShortcuts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_panel(Panel::Shortcuts, window, cx);
    }

    pub(super) fn shortcuts_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let modifier = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        let mut entries = vec![
            ("Arrows".into(), "Move between cells"),
            ("Home / End".into(), "First / last row of the current page"),
            (
                "Page Up / Page Down".into(),
                "Move one screenful within the current page",
            ),
            (
                format!("{modifier}+Left / Right"),
                "First / last visible column",
            ),
            (
                "Shift + navigation keys".into(),
                "Extend the selection within the loaded page",
            ),
            (
                format!("{modifier}+Page Up / Page Down"),
                "Previous / next file page; keep the selected column",
            ),
            ("Alt+Left / Right".into(), "Previous / next file page"),
            (
                format!("{modifier}+Home / End"),
                "First row of the first page / last row of the last page",
            ),
            (
                format!("{modifier}+G"),
                "Go to page: type a page number, then press Enter",
            ),
            (
                format!("{modifier}+A"),
                "Select all loaded rows and visible columns",
            ),
            (
                format!("{modifier}+C"),
                "Copy selected cells (up to 10,000 cells and 1 MiB)",
            ),
            (
                format!("{modifier}+Shift+C"),
                "Copy selected rows with visible column names",
            ),
            ("Enter".into(), "Inspect the selected cell"),
            (
                "Esc".into(),
                "Clear selection or return from a chart / tool",
            ),
            (format!("{modifier}+F"), "Find a column"),
            (format!("{modifier}+O"), "Open a file"),
            ("F1".into(), "Show keyboard shortcuts"),
        ];
        #[cfg(target_os = "macos")]
        entries.push((
            "Cmd+Up / Down".into(),
            "First row of the first page / last row of the last page",
        ));
        div().relative().flex_1().min_h_0()
            .child(div().id("shortcut-list").test_support().size_full().overflow_y_scroll().track_scroll(&self.panel_scroll)
                .child(div().pb_4().text_sm().text_color(cx.omarchy().secondary).child("Table shortcuts leave text-input editing intact. Home/End and screen navigation stay within the current page; file-page shortcuts follow any active filters. Selection spans only the loaded page."))
                .children(entries.drain(..).map(|(keys,description)| div().flex().gap_4().p_3().border_b_1().border_color(cx.omarchy().border)
                    .child(div().w(px(320.0)).flex_shrink_0().text_color(cx.omarchy().accent).child(keys))
                    .child(div().flex_1().child(description)))))
            .child(scrollbar("shortcut-scrollbar", ScrollbarAxis::Vertical, &self.panel_scroll, window, cx))
    }
    pub(super) fn copy_selection(
        &mut self,
        _: &CopySelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copy_selected(false, cx);
    }
    pub(super) fn copy_rows(&mut self, _: &CopyRows, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_selected(true, cx);
    }
    fn copy_selected(&mut self, whole_rows: bool, cx: &mut Context<Self>) {
        let (Some(selection), Some(page)) = (&self.selection, &self.page) else {
            return;
        };
        let count_rows = selection.anchor.0.abs_diff(selection.head.0) + 1;
        let count_columns = if whole_rows || selection.whole_rows {
            self.columns.len()
        } else {
            selection.anchor.1.abs_diff(selection.head.1) + 1
        };
        if count_rows.saturating_mul(count_columns) > 10_000 {
            self.error = Some("Select at most 10,000 cells to copy".into());
            cx.notify();
            return;
        }
        let rows: Vec<_> = (selection.anchor.0.min(selection.head.0)
            ..=selection.anchor.0.max(selection.head.0))
            .map(|r| self.rows[r])
            .collect();
        let whole_rows = whole_rows || selection.whole_rows;
        let columns = if whole_rows {
            self.columns.clone()
        } else {
            self.columns[selection.anchor.1.min(selection.head.1)
                ..=selection.anchor.1.max(selection.head.1)]
                .to_vec()
        };
        match parquetviewer::inspection::copy_cells(&page.frame, &rows, &columns, whole_rows) {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.notice = Some(format!(
                    "Copied {} cells{}",
                    rows.len() * columns.len(),
                    if whole_rows { " with column names" } else { "" }
                ));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    pub(super) fn inspect_selection(
        &mut self,
        _: &InspectSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = &self.selection {
            let (row, col) = selection.head;
            self.inspect_cell(self.rows[row], self.columns[col], window, cx);
        }
    }
    pub(super) fn column_menu(&self, column: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        menu(
            ("column-menu", column),
            control(("column-options", column), "⋯", cx)
                .aria_label("Column details, filter and display options")
                .disabled(self.loading),
            vec![
                MenuItem::new("Column details…"),
                MenuItem::new("Filter…"),
                MenuItem::new("Hide column").disabled(self.columns.len() < 2),
                MenuItem::new("Fit visible values"),
            ],
            move |item, window, cx| {
                owner.update(cx, |this, cx| match item {
                    0 => this.open_detail(column, window, cx),
                    1 => this.open_filter(column, window, cx),
                    2 => this.toggle_column(column, cx),
                    3 => this.fit_column(column, cx),
                    _ => {}
                })
            },
        )
    }
    pub(super) fn open_filter(
        &mut self,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        let saved = self
            .filters
            .iter()
            .find(|f| f.column == page.profiles[column].name)
            .cloned();
        let dtype = page.frame.columns()[column].dtype();
        let scalar = dtype.is_numeric()
            || dtype.is_temporal()
            || matches!(
                dtype,
                polars_core::prelude::DataType::String
                    | polars_core::prelude::DataType::Boolean
                    | polars_core::prelude::DataType::Categorical(..)
                    | polars_core::prelude::DataType::Enum(..)
            );
        self.filter_op = saved.as_ref().map_or(
            if scalar {
                FilterOp::Equals
            } else {
                FilterOp::IsNull
            },
            |f| f.op,
        );
        self.filter_input.update(cx, |input, cx| {
            input.set_value(
                saved.as_ref().map_or_else(
                    || {
                        if matches!(dtype, polars_core::prelude::DataType::Boolean) {
                            "true".into()
                        } else {
                            "".into()
                        }
                    },
                    |f| f.value.clone(),
                ),
                window,
                cx,
            )
        });
        self.filter_upper.update(cx, |input, cx| {
            input.set_value(
                saved.as_ref().map_or("".into(), |f| f.upper.clone()),
                window,
                cx,
            )
        });
        self.show_panel(Panel::Filter(column), window, cx);
    }
    pub(super) fn filter_panel(
        &self,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let page = self.page.as_ref().unwrap();
        let profile = &page.profiles[column];
        let dtype = page.frame.columns()[column].dtype();
        let text = matches!(
            dtype,
            polars_core::prelude::DataType::String
                | polars_core::prelude::DataType::Categorical(..)
                | polars_core::prelude::DataType::Enum(..)
        );
        let scalar = text
            || dtype.is_numeric()
            || dtype.is_temporal()
            || matches!(dtype, polars_core::prelude::DataType::Boolean);
        let boolean = matches!(dtype, polars_core::prelude::DataType::Boolean);
        let has_value = !matches!(self.filter_op, FilterOp::IsNull | FilterOp::NotNull);
        let choices = [
            FilterOp::Contains,
            FilterOp::Equals,
            FilterOp::Range,
            FilterOp::IsNull,
            FilterOp::NotNull,
        ]
        .into_iter()
        .filter(|op| match op {
            FilterOp::Contains => text,
            FilterOp::Equals => scalar,
            FilterOp::Range => dtype.is_numeric() || dtype.is_temporal(),
            _ => true,
        });
        div().flex().flex_col().gap_4()
            .child(div().text_lg().child(format!("{} · {}",profile.name,profile.dtype)))
            .child(div().flex().flex_wrap().gap_2().children(choices.enumerate().map(|(i,op)|control(("filter-op",i),op.label(),cx).selected(self.filter_op==op).on_click(cx.listener(move |this,_,_,cx| {this.filter_op=op;cx.notify();})))))
            .when(has_value && !boolean, |d|d.child(div().flex().flex_col().gap_2().w(px(400.)).max_w_full()
                .child(if self.filter_op==FilterOp::Range { "Lower bound (inclusive)" } else { "Value" })
                .child(input("filter-value",&self.filter_input,window,cx).w_full())))
            .when(self.filter_op==FilterOp::Range, |d|d.child(div().flex().flex_col().gap_2().w(px(400.)).max_w_full()
                .child("Upper bound (inclusive)")
                .child(input("filter-upper",&self.filter_upper,window,cx).w_full())))
            .when(has_value && boolean, |d|d.child(div().flex().gap_2().children([("true", "True"), ("false", "False")].into_iter().enumerate().map(|(i,(value,label))| {
                control(("boolean-filter",i),label,cx)
                    .selected(self.filter_input.read(cx).value().as_str()==value)
                    .on_click(cx.listener(move |this,_,window,cx| {this.filter_input.update(cx,|input,cx| input.set_value(value,window,cx));cx.notify();}))
            }))))
            .when(text && self.filter_op==FilterOp::Equals, |d|d.child(div().text_sm().text_color(cx.omarchy().secondary).child("Leave the value empty to match empty strings.")))
            .when(dtype.is_temporal(),|d|d.child("Dates: YYYY-MM-DD · Timestamps: ISO with timezone, or YYYY-MM-DD HH:MM:SS (UTC) · Time: HH:MM:SS · Duration: integer in its stored unit"))
            .child(div().text_sm().text_color(cx.omarchy().secondary).child("Filters apply to the entire file. Multiple column filters use AND. Text comparisons are case sensitive. Value filters exclude nulls; use Is null to find them."))
            .child(div().text_sm().text_color(cx.omarchy().secondary).child("Scanning runs in the background and can be cancelled. Remote files may require substantial downloads. The total match count becomes available after the scan."))
            .child(control("apply-filter","Apply filter",cx).self_start().on_click(cx.listener(move |this,_,window,cx| {
                let page=this.page.as_ref().unwrap();
                let filter=Filter{column:page.profiles[column].name.clone(),op:this.filter_op,value:this.filter_input.read(cx).value().to_string(),upper:this.filter_upper.read(cx).value().to_string()};
                let mut filters=this.filters.clone(); filters.retain(|f|f.column!=filter.column);filters.push(filter);
                this.run_filters(filters,window,cx);
            })))
    }
    pub(super) fn run_filters(
        &mut self,
        filters: Vec<Filter>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        if let Err(error) = parquetviewer::filter::validate(&filters, &page.frame) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let location = page.source.location.clone();
        self.filter_override = Some(filters);
        self.load_location(location, 0, false, window, cx);
    }
    pub(super) fn filter_chips(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .when(!self.filters.is_empty(), |d| {
                d.mb_3()
                    .child(div().text_xs().self_center().child("Whole-file filters:"))
                    .children(self.filters.iter().enumerate().map(|(i, f)| {
                        control(
                            ("remove-filter", i),
                            format!("{} ×", f.label().chars().take(120).collect::<String>()),
                            cx,
                        )
                        .disabled(self.loading)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                let mut filters = this.filters.clone();
                                filters.remove(i);
                                this.run_filters(filters, window, cx);
                            },
                        ))
                    }))
                    .child(
                        control("clear-filters", "Clear all", cx)
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.run_filters(Vec::new(), window, cx)
                            })),
                    )
            })
    }
    pub(super) fn start_progress_timer(&mut self, cx: &mut Context<Self>) {
        if !self.scanning {
            return;
        }
        self.progress_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let running = this
                    .update(cx, |this, cx| {
                        if this.loading {
                            cx.notify();
                        }
                        this.loading
                    })
                    .unwrap_or(false);
                if !running {
                    break;
                }
            }
        }));
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::{Panel, SettingsStore, Viewer};
    use gpui_kit::{TestAppContext, WindowOptions, point, prelude::*, px, test::TestWindowExt};

    fn shortcut(suffix: &str) -> String {
        format!(
            "{}-{suffix}",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        )
    }

    #[gpui_kit::test]
    fn keyboard_row_screen_column_selection_and_input_scopes(cx: &mut TestAppContext) {
        let file = crate::viewer::tests::fixture(2501);
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        cx.update(crate::viewer::bind_table_keys);
        cx.update(|cx| {
            cx.bind_keys([gpui_kit::KeyBinding::new(
                "escape",
                crate::viewer::CloseDetail,
                Some("ParquetViewer"),
            )])
        });
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            // The table shortcuts also work immediately after opening, before a mouse click.
            window.press("end", cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (499, 0));
            window.render_frame(cx);
            assert!(content.read(cx).vertical.is_scrolled_to_end().unwrap());
            window.press("home", cx);
            window.render_frame(cx);
            assert_eq!(content.read(cx).row_position(), 0);
            window.press("pagedown", cx);
            let row = content.read(cx).selection.as_ref().unwrap().head.0;
            assert!(
                row > 0 && row < 100,
                "Page Down moves a screenful, not a file page"
            );
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 0);
            window.render_frame(cx);
            assert_eq!(
                content.read(cx).row_position(),
                row,
                "Page Down must move the viewport on its first press"
            );
            window.press("pageup", cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head.0, 0);
            window.press("shift-pagedown", cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().anchor, (0, 0));
            window.press("shift-end", cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head.0, 499);
            window.press("home", cx);
            window.press(&shortcut("right"), cx);
            window.render_frame(cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (0, 39));
            assert!(window.find(("column", 39usize)).visible());
            window.press(&shortcut("shift-left"), cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().anchor, (0, 39));
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (0, 0));
            window.press(&shortcut("a"), cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (499, 39));
            window.press(&shortcut("c"), cx);
            assert!(content.read(cx).error.as_ref().unwrap().contains("10,000"));
            window.press("escape", cx);
            assert!(content.read(cx).selection.is_none());
            window.press("home", cx);
            window.press(&shortcut("g"), cx);
            assert_eq!(
                content
                    .read(cx)
                    .page_input
                    .read(cx)
                    .selected_value()
                    .as_str(),
                "1"
            );
            window.input("4", cx);
            // Editing shortcuts belong to the input, and file-navigation keys do not escape it.
            window.press("home", cx);
            window.press("end", cx);
            window.press(&shortcut("a"), cx);
            assert_eq!(
                content
                    .read(cx)
                    .page_input
                    .read(cx)
                    .selected_value()
                    .as_str(),
                "4"
            );
            // Option+Right is native word navigation on macOS and can collapse selection.
            window.press("alt-right", cx);
            window.press(&shortcut("pagedown"), cx);
            assert!(!content.read(cx).loading);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 0);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (0, 0));
            assert_eq!(content.read(cx).page_input.read(cx).value().as_str(), "4");
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().page_number(), 4);
            window.press("end", cx);
            assert_eq!(
                content.read(cx).selection.as_ref().unwrap().head.0,
                499,
                "Go to page must return focus to the table"
            );
            // Tool inputs keep their own keys and cannot navigate the underlying table.
            content.update(cx, |this, cx| {
                this.find_column(&crate::viewer::FindColumn, window, cx)
            });
            window.render_frame(cx);
            window.input("column_1", cx);
            window.press("end", cx);
            window.press(&shortcut("end"), cx);
            window.press("alt-left", cx);
            assert!(matches!(content.read(cx).panel, Some(Panel::Find)));
            assert_eq!(content.read(cx).page.as_ref().unwrap().page_number(), 4);
            window.press("escape", cx);
            window.press("f1", cx);
            window.render_frame(cx);
            assert!(matches!(content.read(cx).panel, Some(Panel::Shortcuts)));
            assert!(window.find("shortcut-list").visible());
            window.press(&shortcut("home"), cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().page_number(), 4);
            window.press("escape", cx);
            assert!(content.read(cx).panel.is_none());
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn keyboard_file_pages_preserve_column_and_respect_filters_and_all(cx: &mut TestAppContext) {
        let file = crate::viewer::tests::fixture(2501);
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        cx.update(crate::viewer::bind_table_keys);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(("cell", 2usize), cx);
            window.press(&shortcut("pagedown"), cx);
            // Repeated keys during a read cannot launch another concurrent decode.
            window.press(&shortcut("end"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 500);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (0, 2));
            window.press(&shortcut("pageup"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 0);
            window.press("alt-left", cx);
            assert!(!content.read(cx).loading);
            window.press(&shortcut("end"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 2500);
            assert_eq!(content.read(cx).rows.len(), 1);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head, (0, 2));
            window.press("alt-right", cx);
            assert!(!content.read(cx).loading);
            window.press(&shortcut("home"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 0);
            content.update(cx, |this, cx| {
                this.run_filters(
                    vec![parquetviewer::filter::Filter {
                        column: "column_0".into(),
                        op: parquetviewer::filter::FilterOp::Range,
                        value: "50".into(),
                        upper: "600".into(),
                    }],
                    window,
                    cx,
                )
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press(&shortcut("end"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                content.read(cx).page.as_ref().unwrap().matching_rows,
                Some(551)
            );
            assert_eq!(content.read(cx).rows.len(), 51);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head.0, 50);
            window.click(("page-size", 3usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press(&shortcut("end"), cx);
            assert_eq!(content.read(cx).rows.len(), 551);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head.0, 550);
            assert!(
                !content.read(cx).loading,
                "All is one page; no read is needed for a keyboard jump"
            );
            window.press(&shortcut("home"), cx);
            assert_eq!(content.read(cx).selection.as_ref().unwrap().head.0, 0);
            content.update(cx, |this, cx| {
                this.run_filters(
                    vec![parquetviewer::filter::Filter {
                        column: "column_0".into(),
                        op: parquetviewer::filter::FilterOp::Equals,
                        value: "9999".into(),
                        upper: String::new(),
                    }],
                    window,
                    cx,
                )
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(content.read(cx).rows.is_empty());
            for key in [
                "home".to_string(),
                "end".into(),
                shortcut("end"),
                shortcut("pagedown"),
                shortcut("a"),
            ] {
                window.press(&key, cx);
            }
            assert!(content.read(cx).selection.is_none());
            assert!(!content.read(cx).loading);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn native_selection_resize_visibility_and_clipboard_restore(cx: &mut TestAppContext) {
        let file = crate::viewer::tests::fixture(251);
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        cx.update(crate::viewer::bind_table_keys);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(("cell", 0usize), cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-c"
                },
                cx,
            );
            assert_eq!(cx.read_from_clipboard().unwrap().text(), Some("250".into()));
            window.press("shift-right", cx);
            window.press("shift-down", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-c"
                },
                cx,
            );
            assert_eq!(
                cx.read_from_clipboard().unwrap().text(),
                Some("250\t250\n249\t249".into())
            );
            window.press("enter", cx);
            window.render_frame(cx);
            assert!(matches!(content.read(cx).panel, Some(Panel::Cell(_))));
            window.click("close-panel", cx);
            window.render_frame(cx);
            let edge = window.find(("resize-column", 0usize)).bounds().center();
            window.drag(edge, point(edge.x + px(140.0), edge.y), cx);
            window.render_frame(cx);
            assert!((content.read(cx).column_width(0) - 360.0).abs() < 2.0);
            assert_eq!(
                window.find(("column", 1usize)).bounds().left(),
                window.find(("cell", 1usize)).bounds().left()
            );
            window.double_click(("resize-column", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(content.read(cx).column_width(0), 220.0);
            window.click(("source-row", 0usize), cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-shift-c"
                } else {
                    "ctrl-shift-c"
                },
                cx,
            );
            assert!(
                cx.read_from_clipboard()
                    .unwrap()
                    .text()
                    .unwrap()
                    .starts_with("column_0\tcolumn_1")
            );
            content.update(cx, |this, cx| this.show_panel(Panel::Columns, window, cx));
            window.render_frame(cx);
            window.click(("visible-column", 1usize), cx);
            assert!(!content.read(cx).columns.contains(&1));
            window.click("close-panel", cx);
            window.render_frame(cx);
            assert!(window.try_find(("column", 1usize)).is_none());
            assert_eq!(content.read(cx).columns[1], 2);
            content.update(cx, |this, cx| {
                this.set_width(2, 400.0, cx);
                this.save_settings(cx);
            });
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        let file_view = store.load().unwrap().files.remove(0);
        assert_eq!(file_view.hidden_columns, vec!["column_1"]);
        assert_eq!(file_view.widths["column_2"], 400.0);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).columns[1], 2);
            assert_eq!(content.read(cx).column_width(2), 400.0);
            window.remove_window();
        })
        .unwrap();
    }
    #[gpui_kit::test]
    fn whole_file_filter_paging_scope_saved_views_and_clear(cx: &mut TestAppContext) {
        let file = crate::viewer::tests::fixture(2501);
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |this, cx| this.open_filter(0, window, cx));
            window.render_frame(cx);
            window.click(("filter-op", 1usize), cx); // numeric: Equals, Range, null, not-null
            content.update(cx, |this, cx| {
                this.filter_input
                    .update(cx, |input, cx| input.set_value("50", window, cx));
                this.filter_upper
                    .update(cx, |input, cx| input.set_value("600", window, cx));
            });
            window.render_frame(cx);
            window.click("apply-filter", cx);
            assert!(content.read(cx).loading);
            assert!(content.read(cx).scanning);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let view = content.read(cx);
            assert!(!view.loading, "{:?}", view.error);
            assert_eq!(view.page.as_ref().unwrap().matching_rows, Some(551));
            assert_eq!(view.page.as_ref().unwrap().source_row(0), 1900);
            assert_eq!(view.page.as_ref().unwrap().page_count(), 2);
            assert!(window.find(("remove-filter", 0usize)).visible());
            window.click("next", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).rows.len(), 51);
            assert_eq!(content.read(cx).page.as_ref().unwrap().source_row(0), 2400);
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().files[0].filters.len(), 1);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).rows.len(), 51);
            window.click("clear-filters", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert!(content.read(cx).filters.is_empty());
            assert!(content.read(cx).filtered.is_none());
            assert_eq!(content.read(cx).rows.len(), 500);
            assert_eq!(content.read(cx).page.as_ref().unwrap().source_row(0), 0);
            window.remove_window();
        })
        .unwrap();
    }
    #[gpui_kit::test]
    fn cancelled_filters_keep_prior_view_and_stale_filters_do_not_block_open(
        cx: &mut TestAppContext,
    ) {
        let file = crate::viewer::tests::fixture(2501);
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(dir.path().join("settings.json"));
        let path = std::fs::canonicalize(file.path()).unwrap();
        let mut settings = parquetviewer::settings::Settings::default();
        settings.remember(parquetviewer::settings::FileView {
            path: path.clone(),
            filters: vec![parquetviewer::filter::Filter {
                column: "deleted_column".into(),
                op: parquetviewer::filter::FilterOp::Equals,
                value: "1".into(),
                upper: String::new(),
            }],
            ..Default::default()
        });
        store.save(&settings).unwrap();
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(path), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).rows.len(), 500);
            assert!(content.read(cx).filters.is_empty());
            assert!(
                content
                    .read(cx)
                    .notice
                    .as_ref()
                    .unwrap()
                    .contains("schema changed")
            );
            content.update(cx, |this, cx| {
                this.run_filters(
                    vec![parquetviewer::filter::Filter {
                        column: "column_0".into(),
                        op: parquetviewer::filter::FilterOp::IsNull,
                        value: String::new(),
                        upper: String::new(),
                    }],
                    window,
                    cx,
                )
            });
            window.render_frame(cx);
            window.click("cancel-load", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).rows.len(), 500);
            assert!(content.read(cx).filters.is_empty());
            assert!(
                content
                    .read(cx)
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Cancelled")
            );
            assert!(!content.read(cx).loading);
            window.remove_window();
        })
        .unwrap();
        assert!(store.load().unwrap().files[0].filters.is_empty());
    }
}
