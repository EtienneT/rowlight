use std::{
    ops::Range,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

mod tools;
use tools::Selection;

#[derive(Clone, Copy, Debug, PartialEq)]
enum TableDestination {
    FirstRow,
    LastRow,
    ScreenUp,
    ScreenDown,
    FirstColumn,
    LastColumn,
    PreviousPage,
    NextPage,
    FirstPage,
    LastPage,
}

#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = parquetviewer, no_json)]
struct TableNavigation {
    destination: TableDestination,
    extend: bool,
}

// Input controls retain their own navigation and selection shortcuts.
const TABLE_CONTEXT: &str = "TableView && !Input";

use gpui_kit::base::{
    ElementExt, PaginationState, ScrollbarAxis,
    input::{InputEvent, InputState, TextareaState},
};
use gpui_kit::{prelude::*, *};
use gpui_omarchy::{
    ActiveTheme, ButtonVariant, MenuItem, button, input, menu, pagination, scrollbar,
};
use parquetviewer::{
    association,
    data::{self, ColumnDetail, Distribution, PAGE_SIZES, Page, RowLimit},
    filter::{Filter, FilterIndex, FilterOp},
    inspection::CellKind,
    settings::{FileView, Settings, SettingsStore, Sort},
    source::{Cancellation, CsvOptions, Delimiter, Location, Source},
};

actions!(
    parquetviewer,
    [
        OpenFile,
        FindColumn,
        PreviousPage,
        NextPage,
        CloseDetail,
        Quit,
        CopySelection,
        CopyRows,
        InspectSelection,
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        ExtendLeft,
        ExtendRight,
        ExtendUp,
        ExtendDown,
        SelectAllCells,
        FocusPage,
        ShowShortcuts
    ]
);

pub(crate) fn bind_table_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-c"
            } else {
                "ctrl-c"
            },
            CopySelection,
            Some(TABLE_CONTEXT),
        ),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-shift-c"
            } else {
                "ctrl-shift-c"
            },
            CopyRows,
            Some(TABLE_CONTEXT),
        ),
        KeyBinding::new("enter", InspectSelection, Some(TABLE_CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(TABLE_CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(TABLE_CONTEXT)),
        KeyBinding::new("up", MoveUp, Some(TABLE_CONTEXT)),
        KeyBinding::new("down", MoveDown, Some(TABLE_CONTEXT)),
        KeyBinding::new("shift-left", ExtendLeft, Some(TABLE_CONTEXT)),
        KeyBinding::new("shift-right", ExtendRight, Some(TABLE_CONTEXT)),
        KeyBinding::new("shift-up", ExtendUp, Some(TABLE_CONTEXT)),
        KeyBinding::new("shift-down", ExtendDown, Some(TABLE_CONTEXT)),
    ]);
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let mut keys = Vec::new();
    for (key, destination) in [
        ("home", TableDestination::FirstRow),
        ("end", TableDestination::LastRow),
        ("pageup", TableDestination::ScreenUp),
        ("pagedown", TableDestination::ScreenDown),
    ] {
        for extend in [false, true] {
            keys.push(KeyBinding::new(
                &if extend {
                    format!("shift-{key}")
                } else {
                    key.into()
                },
                TableNavigation {
                    destination,
                    extend,
                },
                Some(TABLE_CONTEXT),
            ));
        }
    }
    for (key, destination) in [
        ("left", TableDestination::FirstColumn),
        ("right", TableDestination::LastColumn),
    ] {
        for extend in [false, true] {
            keys.push(KeyBinding::new(
                &format!("{modifier}-{}{key}", if extend { "shift-" } else { "" }),
                TableNavigation {
                    destination,
                    extend,
                },
                Some(TABLE_CONTEXT),
            ));
        }
    }
    for (key, destination) in [
        (format!("{modifier}-pageup"), TableDestination::PreviousPage),
        (format!("{modifier}-pagedown"), TableDestination::NextPage),
        ("alt-left".into(), TableDestination::PreviousPage),
        ("alt-right".into(), TableDestination::NextPage),
        (format!("{modifier}-home"), TableDestination::FirstPage),
        (format!("{modifier}-end"), TableDestination::LastPage),
    ] {
        keys.push(KeyBinding::new(
            &key,
            TableNavigation {
                destination,
                extend: false,
            },
            Some(TABLE_CONTEXT),
        ));
    }
    #[cfg(target_os = "macos")]
    for (key, destination) in [
        ("cmd-up", TableDestination::FirstPage),
        ("cmd-down", TableDestination::LastPage),
    ] {
        keys.push(KeyBinding::new(
            key,
            TableNavigation {
                destination,
                extend: false,
            },
            Some(TABLE_CONTEXT),
        ));
    }
    keys.push(KeyBinding::new(
        &format!("{modifier}-a"),
        SelectAllCells,
        Some(TABLE_CONTEXT),
    ));
    keys.push(KeyBinding::new(
        &format!("{modifier}-g"),
        FocusPage,
        Some(TABLE_CONTEXT),
    ));
    keys.push(KeyBinding::new("f1", ShowShortcuts, Some("ParquetViewer")));
    cx.bind_keys(keys);
}

const COLUMN_WIDTH: f32 = 220.0;
const ROW_NUMBER_WIDTH: f32 = 80.0;
const ROW_HEIGHT: f32 = 32.0;

// Chart identities stay unique even when two bounded category labels match.
#[derive(Clone, PartialEq, Eq, Hash)]
struct PlotBand {
    index: usize,
    label: String,
}
impl From<PlotBand> for SharedString {
    fn from(band: PlotBand) -> Self {
        band.label.into()
    }
}

enum Panel {
    Find,
    Cell(data::CellDetail),
    Info,
    Recent,
    Url,
    Csv,
    Overview,
    Columns,
    Filter(usize),
    Shortcuts,
}

pub struct Viewer {
    page: Option<Arc<Page>>,
    columns: Vec<usize>,
    rows: Vec<usize>,
    sort: Option<(usize, bool)>,
    settings: Settings,
    store: SettingsStore,
    save_task: Option<Task<()>>,
    loading: bool,
    error: Option<String>,
    focus: FocusHandle,
    horizontal: ScrollHandle,
    viewport_width: Pixels,
    viewport_x: Pixels,
    vertical: UniformListScrollHandle,
    page_input: Entity<InputState>,
    _input_subscription: Subscription,
    _bounds_subscription: Subscription,
    load_task: Option<Task<()>>,
    dialog_task: Option<Task<()>>,
    detail: Option<ColumnDetail>,
    association_offer: bool,
    association_busy: bool,
    association_task: Option<Task<()>>,
    notice: Option<String>,
    panel: Option<Panel>,
    search_input: Entity<InputState>,
    url_input: Entity<InputState>,
    profile_input: Entity<InputState>,
    text_area: Entity<TextareaState>,
    _search_subscription: Subscription,
    panel_scroll: ScrollHandle,
    operation: Cancellation,
    pending_source: Option<Location>,
    attempted_source: Option<Location>,
    overview: Option<Arc<data::FileOverview>>,
    detail_scope: Option<String>,
    history_suppressed: bool,
    find_scroll: UniformListScrollHandle,
    csv_draft: CsvOptions,
    csv_target: Option<Location>,
    csv_override: Option<CsvOptions>,
    widths: Vec<f32>,
    hidden_columns: Vec<String>,
    grid_focus: FocusHandle,
    selection: Option<Selection>,
    resizing: Option<(usize, f32, f32)>,
    filters: Vec<Filter>,
    filtered: Option<Arc<FilterIndex>>,
    filter_override: Option<Vec<Filter>>,
    filter_op: FilterOp,
    filter_input: Entity<InputState>,
    filter_upper: Entity<InputState>,
    scan_progress: Arc<AtomicUsize>,
    scan_total: Arc<AtomicUsize>,
    scanning: bool,
    progress_task: Option<Task<()>>,
}

impl Viewer {
    pub fn new(
        path: Option<PathBuf>,
        store: SettingsStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (settings, error) = match store.load() {
            Ok(settings) => (settings, None),
            Err(error) => (
                Settings::default(),
                Some(format!("{error:#}; using defaults.")),
            ),
        };
        cx.on_release(|this, _| {
            this.flush_settings();
        })
        .detach();
        cx.on_app_quit(|this, _| {
            this.flush_settings();
            async {}
        })
        .detach();
        let page_input = cx.new(|cx| InputState::new(window, cx).placeholder("Page"));
        let subscription = cx.subscribe_in(&page_input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.jump(window, cx);
            }
        });
        let bounds_subscription = cx.observe_window_bounds(window, |this, window, cx| {
            // A resize must invalidate the retained header as well as the virtual rows.
            this.viewport_width = (window.viewport_size().width - px(44.0)).max(px(0.0));
            if !window.is_fullscreen() && !window.is_maximized() {
                let size = window.viewport_size();
                this.settings.window_width = f32::from(size.width);
                this.settings.window_height = f32::from(size.height);
                this.schedule_save(cx);
            }
            cx.notify();
        });
        let search_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search column names or data types"));
        let url_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("s3://bucket/file.parquet or https://…")
        });
        let profile_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("AWS_PROFILE / default")
                .default_value(settings.aws_profile.clone())
        });
        let text_area = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx).rows(18);
            input.set_readonly(true, cx);
            input
        });
        let search_subscription = cx.subscribe(&search_input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let filter_input = cx.new(|cx| InputState::new(window, cx).placeholder("Value"));
        let filter_upper = cx.new(|cx| InputState::new(window, cx).placeholder("Upper bound"));
        let association_offer = !settings.association_prompted;
        let mut viewer = Self {
            page: None,
            columns: Vec::new(),
            rows: Vec::new(),
            sort: None,
            settings,
            store,
            save_task: None,
            loading: false,
            error,
            focus: cx.focus_handle(),
            horizontal: ScrollHandle::new(),
            viewport_width: (window.viewport_size().width - px(44.0)).max(px(0.0)),
            viewport_x: px(0.0),
            vertical: UniformListScrollHandle::new(),
            page_input,
            _input_subscription: subscription,
            _bounds_subscription: bounds_subscription,
            load_task: None,
            dialog_task: None,
            detail: None,
            association_offer,
            association_busy: false,
            association_task: None,
            notice: None,
            panel: None,
            search_input,
            url_input,
            profile_input,
            text_area,
            _search_subscription: search_subscription,
            panel_scroll: ScrollHandle::new(),
            operation: Cancellation::default(),
            pending_source: None,
            attempted_source: None,
            overview: None,
            detail_scope: None,
            history_suppressed: false,
            find_scroll: UniformListScrollHandle::new(),
            csv_draft: CsvOptions::default(),
            csv_target: None,
            csv_override: None,
            widths: Vec::new(),
            hidden_columns: Vec::new(),
            grid_focus: cx.focus_handle(),
            selection: None,
            resizing: None,
            filters: Vec::new(),
            filtered: None,
            filter_override: None,
            filter_op: FilterOp::Equals,
            filter_input,
            filter_upper,
            scan_progress: Arc::new(AtomicUsize::new(0)),
            scan_total: Arc::new(AtomicUsize::new(0)),
            scanning: false,
            progress_task: None,
        };
        viewer.focus.focus(window, cx);
        if let Some(path) = path {
            viewer.open_path(path, window, cx);
        }
        viewer
    }

    fn flush_settings(&mut self) {
        self.remember_view();
        if let Err(error) = self.store.save(&self.settings) {
            eprintln!("Cannot save preferences: {error:#}");
        }
    }

    fn row_position(&self) -> usize {
        let state = self.vertical.0.borrow();
        state.deferred_scroll_to_item.as_ref().map_or_else(
            || ((-f32::from(state.base_handle.offset().y)).max(0.0) / ROW_HEIGHT) as usize,
            |scroll| scroll.item_index,
        )
    }

    fn remember_view(&mut self) {
        if !self.history_suppressed
            && let Some(page) = &self.page
        {
            self.settings.remember(FileView {
                location: Some(page.source.location.clone()),
                csv_options: Some(page.source.csv_options),
                path: page.path.clone(),
                offset: page.offset,
                sort: self.sort.map(|(column, descending)| Sort {
                    column: page.profiles[column].name.clone(),
                    descending,
                }),
                horizontal: f32::from(self.horizontal.offset().x),
                row: self.row_position(),
                widths: page
                    .profiles
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.name.clone(), self.column_width(i)))
                    .collect(),
                hidden_columns: self.hidden_columns.clone(),
                filters: self.filters.clone(),
            });
        }
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        self.remember_view();
        if let Err(error) = self.store.save(&self.settings) {
            self.error = Some(format!("Cannot save preferences: {error:#}"));
            cx.notify();
        }
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.remember_view();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |this, cx| this.save_settings(cx));
        }));
    }

    pub(crate) fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let location = match path.to_str().filter(|s| s.contains("://")) {
            Some(value) => match Location::remote(value) {
                Ok(location) => location,
                Err(error) => {
                    self.panel = None;
                    self.close_detail(window, cx);
                    self.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            },
            None => Location::local(path),
        };
        self.open_source(location, window, cx);
    }

    fn open_source(&mut self, location: Location, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            self.pending_source = Some(location);
            self.operation.cancel();
            return;
        }
        self.remember_view();
        self.history_suppressed = false;
        let offset = self
            .settings
            .file_location(&location)
            .map_or(0, |file| file.offset);
        self.load_location(location, offset, true, window, cx);
    }

    fn set_row_limit(&mut self, limit: RowLimit, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.settings.row_limit == limit {
            return;
        }
        self.settings.row_limit = limit;
        self.save_settings(cx);
        if let Some(page) = &self.page {
            self.load_location(page.source.location.clone(), page.offset, false, window, cx);
        }
        cx.notify();
    }

    fn load_location(
        &mut self,
        location: Location,
        offset: usize,
        fresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.load_location_selected(location, offset, fresh, None, window, cx);
    }

    fn load_location_selected(
        &mut self,
        location: Location,
        offset: usize,
        fresh: bool,
        keyboard_selection: Option<(bool, usize)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // One outstanding decode per window keeps synchronous worker memory bounded.
        if self.loading {
            return;
        }
        self.close_detail(window, cx);
        self.panel = None;
        self.overview = None;
        self.attempted_source = Some(location.clone());
        self.operation = Cancellation::default();
        let cancel = self.operation.clone();
        let source = self
            .page
            .as_ref()
            .filter(|page| !fresh && page.source.location == location)
            .map(|p| Arc::clone(&p.source));
        let csv_options = self.csv_override.take().unwrap_or_else(|| {
            self.settings
                .file_location(&location)
                .and_then(|f| f.csv_options)
                .unwrap_or(self.settings.csv_options)
        });
        let filters = self.filter_override.take().unwrap_or_else(|| {
            if !fresh
                && self
                    .page
                    .as_ref()
                    .is_some_and(|p| p.source.location == location)
            {
                self.filters.clone()
            } else {
                self.settings
                    .file_location(&location)
                    .map_or_else(Vec::new, |f| f.filters.clone())
            }
        });
        let filtered = self
            .filtered
            .as_ref()
            .filter(|index| !fresh && index.source.location == location && index.filters == filters)
            .cloned();
        self.scanning = !filters.is_empty() && filtered.is_none();
        self.scan_progress = Arc::new(AtomicUsize::new(0));
        let progress = self.scan_progress.clone();
        self.scan_total = Arc::new(AtomicUsize::new(0));
        let scan_total = self.scan_total.clone();
        let aws_profile = self.settings.aws_profile.clone();
        self.loading = true;
        self.error = None;
        self.remember_view();
        let row_limit = self.settings.row_limit;
        let saved_sort = self
            .settings
            .file_location(&location)
            .and_then(|file| file.sort.clone());
        let retained_bytes = self
            .page
            .as_ref()
            .map_or(0, |page| page.frame.estimated_size() as u64)
            .saturating_add((self.rows.len() as u64).saturating_mul(8));
        window.set_window_title("Parquet Viewer · Loading…");
        cx.notify();
        self.start_progress_timer(cx);
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let source = match source {
                        Some(source) => source,
                        None => Source::open(location, csv_options, &aws_profile, &cancel)?,
                    };
                    let mut filters = filters;
                    let mut filter_notice = None;
                    if fresh && !filters.is_empty() {
                        let schema = source.read_slice(0, 0, &cancel)?;
                        if let Err(error) = parquetviewer::filter::validate(&filters, &schema) {
                            filter_notice = Some(format!(
                                "Saved filters were cleared because the schema changed: {error}"
                            ));
                            filters.clear();
                        }
                    }
                    scan_total.store(source.info.total_rows, Ordering::Release);
                    let index = if filters.is_empty() {
                        None
                    } else {
                        Some(match filtered {
                            Some(index) => index,
                            None => Arc::new(FilterIndex::build(
                                source.clone(),
                                filters,
                                &cancel,
                                &progress,
                            )?),
                        })
                    };
                    let page = match &index {
                        Some(index) => index.load(offset, row_limit, retained_bytes, &cancel)?,
                        None => {
                            data::load_source(source, offset, row_limit, retained_bytes, &cancel)?
                        }
                    };
                    let sort = saved_sort.and_then(|sort| {
                        page.profiles
                            .iter()
                            .position(|profile| profile.name == sort.column && profile.sortable)
                            .map(|column| (column, sort.descending))
                    });
                    let rows = match sort {
                        Some((column, descending)) => page.sorted_rows(column, descending)?,
                        None => (0..page.frame.height()).collect(),
                    };
                    Ok::<_, anyhow::Error>((page, rows, sort, index, filter_notice))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                this.scanning = false;
                this.notice = None;
                match result {
                    Ok((page, rows, sort, index, filter_notice)) => {
                        this.notice = filter_notice;
                        let changed_file = this.page.as_ref().is_none_or(|previous| {
                            previous.source.location != page.source.location
                        });
                        let saved = this.settings.file_location(&page.source.location).cloned();
                        if changed_file {
                            this.viewport_x =
                                px(saved.as_ref().map_or(0.0, |file| file.horizontal));
                            this.horizontal.set_offset(point(this.viewport_x, px(0.0)));
                        }
                        window.set_window_title(&format!(
                            "{} · Parquet Viewer",
                            page.source.location.name()
                        ));
                        if changed_file || fresh {
                            this.hidden_columns = saved
                                .as_ref()
                                .map_or_else(Vec::new, |f| f.hidden_columns.clone());
                            this.widths = page
                                .profiles
                                .iter()
                                .map(|p| {
                                    saved
                                        .as_ref()
                                        .and_then(|f| f.widths.get(&p.name))
                                        .copied()
                                        .unwrap_or(COLUMN_WIDTH)
                                })
                                .collect();
                        }
                        this.columns = page
                            .profiles
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| !this.hidden_columns.contains(&p.name))
                            .map(|(i, _)| i)
                            .collect();
                        if this.columns.is_empty() && !page.profiles.is_empty() {
                            this.hidden_columns.clear();
                            this.columns = (0..page.profiles.len()).collect();
                        }
                        this.filters = index.as_ref().map_or_else(Vec::new, |i| i.filters.clone());
                        this.filtered = index;
                        this.selection = None;
                        this.rows = rows;
                        this.sort = sort;
                        this.page_input.update(cx, |input, cx| {
                            input.set_value(page.page_number().to_string(), window, cx)
                        });
                        this.page = Some(Arc::new(page));
                        this.vertical.scroll_to_item_strict(
                            if changed_file {
                                saved
                                    .map_or(0, |file| file.row)
                                    .min(this.rows.len().saturating_sub(1))
                            } else {
                                0
                            },
                            ScrollStrategy::Top,
                        );
                        if let Some((at_end, column)) = keyboard_selection {
                            let row = if at_end {
                                this.rows.len().saturating_sub(1)
                            } else {
                                0
                            };
                            this.reveal_cell(
                                row,
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
                        this.save_settings(cx);
                    }
                    Err(error) => {
                        // Keep the prior page usable if All was rejected or decoding failed.
                        if let Some(page) = &this.page {
                            this.settings.row_limit = page.row_limit;
                        } else if this.settings.row_limit == RowLimit::All {
                            this.settings.row_limit = RowLimit::default();
                        }
                        this.save_settings(cx);
                        this.error = Some(format!("{error:#}"));
                        window.set_window_title("Parquet Viewer");
                    }
                }
                if let Some(location) = this.pending_source.take() {
                    this.open_source(location, window, cx);
                }
                cx.notify();
            });
        }));
    }

    fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Parquet, IPC, CSV or TSV file".into()),
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
                        this.error = Some(format!("Cannot open file picker: {error}"));
                    }
                    Err(error) => {
                        this.error = Some(format!("File picker closed: {error}"));
                    }
                }
                cx.notify();
            });
        }));
    }

    fn navigate(&mut self, page_number: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            let number = page_number.clamp(1, page.page_count());
            if number == page.page_number() {
                return;
            }
            self.load_location(
                page.source.location.clone(),
                (number - 1) * page.page_size,
                false,
                window,
                cx,
            );
        }
    }

    fn previous(&mut self, _: &PreviousPage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            self.navigate(page.page_number().saturating_sub(1), window, cx);
        }
    }

    fn next(&mut self, _: &NextPage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            self.navigate(page.page_number() + 1, window, cx);
        }
    }

    fn jump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        match self.page_input.read(cx).value().trim().parse::<usize>() {
            Ok(number) if number > 0 => {
                let Some(page) = &self.page else {
                    return;
                };
                let number = number.clamp(1, page.page_count());
                let current = page.page_number();
                let offset = (number - 1) * page.page_size;
                let location = page.source.location.clone();
                let column = self.selection.as_ref().map_or(0, |s| s.head.1);
                self.page_input.update(cx, |input, cx| {
                    input.set_value(number.to_string(), window, cx)
                });
                if number == current {
                    self.grid_focus.focus(window, cx);
                    cx.notify();
                } else {
                    self.load_location_selected(
                        location,
                        offset,
                        false,
                        Some((false, column)),
                        window,
                        cx,
                    );
                }
            }
            _ => {
                self.error = Some("Enter a page number of 1 or greater.".into());
                cx.notify();
            }
        }
    }

    fn sort_column(&mut self, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = &self.page else {
            return;
        };
        if self.loading || !page.profiles[column].sortable {
            return;
        }
        let next = match self.sort {
            Some((current, false)) if current == column => Some((column, true)),
            Some((current, true)) if current == column => None,
            _ => Some((column, false)),
        };
        if self.rows.len() > data::PROFILE_ROWS {
            let page = Arc::clone(page);
            self.operation = Cancellation::default();
            let cancel = self.operation.clone();
            self.loading = true;
            cx.notify();
            self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
                let result = cx
                    .background_spawn(async move {
                        cancel.check()?;
                        let result = match next {
                            Some((column, descending)) => page.sorted_rows(column, descending),
                            None => Ok((0..page.frame.height()).collect()),
                        };
                        cancel.check()?;
                        result
                    })
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    this.loading = false;
                    match result {
                        Ok(rows) => {
                            this.rows = rows;
                            this.sort = next;
                            this.selection = None;
                            this.vertical.scroll_to_item_strict(0, ScrollStrategy::Top);
                            this.save_settings(cx);
                        }
                        Err(error) => this.error = Some(error.to_string()),
                    }
                    this.notice = None;
                    if let Some(location) = this.pending_source.take() {
                        this.open_source(location, window, cx);
                    }
                    cx.notify();
                });
            }));
            return;
        }
        if let Some((column, descending)) = next {
            match page.sorted_rows(column, descending) {
                Ok(rows) => self.rows = rows,
                Err(error) => {
                    self.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            }
        } else {
            self.rows = (0..page.frame.height()).collect();
        }
        self.sort = next;
        self.selection = None;
        self.vertical.scroll_to_item_strict(0, ScrollStrategy::Top);
        self.save_settings(cx);
        cx.notify();
    }

    /// Render only columns intersecting the viewport, with one column of overdraw.
    fn visible_columns(&self) -> Range<usize> {
        let x = f32::from(-self.viewport_x).max(0.0);
        let width = f32::from(self.viewport_width).max(1.0);
        let mut at = ROW_NUMBER_WIDTH;
        let mut start = 0;
        while start < self.columns.len() && at + self.column_width(self.columns[start]) < x {
            at += self.column_width(self.columns[start]);
            start += 1;
        }
        let mut end = start;
        while end < self.columns.len() && at < x + width {
            at += self.column_width(self.columns[end]);
            end += 1;
        }
        start..(end + 1).min(self.columns.len())
    }

    fn row(&self, row_ix: usize, cx: &App, owner: Entity<Self>) -> AnyElement {
        let Some(page) = &self.page else {
            return div().id(("row", row_ix)).into_any_element();
        };
        let row = self.rows[row_ix];
        let visible = self.visible_columns();
        let row_owner = owner.clone();
        let mut result = div()
            .id(("row", row_ix))
            .test_support()
            .flex()
            .h(px(ROW_HEIGHT))
            .w_full()
            .text_sm()
            .border_b_1()
            .border_color(cx.omarchy().border.opacity(0.5))
            .when(row_ix % 2 == 1, |d| {
                d.bg(cx.omarchy().surface.opacity(0.25))
            })
            .child(
                div()
                    .id(("source-row", row_ix))
                    .test_support()
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        row_owner.update(cx, |this, cx| {
                            this.select_cell(row_ix, 0, event.modifiers.shift, true, window, cx)
                        })
                    })
                    .w(px(ROW_NUMBER_WIDTH))
                    .flex_shrink_0()
                    .px_3()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(cx.omarchy().secondary)
                    .justify_end()
                    .bg(cx.omarchy().surface)
                    .border_r_1()
                    .border_color(cx.omarchy().border)
                    .child(number(page.source_row(row) + 1)),
            )
            .child(
                div()
                    .w(px(self.columns_width(0..visible.start)))
                    .flex_shrink_0(),
            );
        for ix in visible.clone() {
            let column = self.columns[ix];
            let (mut value, _) = page.cell(row, column);
            let kind = page.frame.columns()[column]
                .get(row)
                .map(|v| parquetviewer::inspection::cell_kind(&v))
                .unwrap_or(CellKind::Value);
            if kind == CellKind::Empty {
                value = "(empty string)".into();
            }
            let selected = self
                .selection
                .as_ref()
                .is_some_and(|s| s.contains(row_ix, ix));
            let cell_owner = owner.clone();
            let drag_owner = owner.clone();
            result = result.child(
                div()
                    .id(("cell", row_ix * page.profiles.len() + column))
                    .test_support()
                    .w(px(self.column_width(column)))
                    .flex_shrink_0()
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .border_r_1()
                    .border_color(cx.omarchy().border.opacity(0.5))
                    .overflow_hidden()
                    .hover(|d| d.bg(cx.omarchy().accent.opacity(0.08)))
                    .when(selected, |d| d.bg(cx.omarchy().accent.opacity(0.2)))
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        cell_owner.update(cx, |this, cx| {
                            this.select_cell(row_ix, ix, event.modifiers.shift, false, window, cx);
                            if event.click_count == 2 {
                                this.inspect_cell(row, column, window, cx);
                            }
                        });
                    })
                    .on_mouse_move(move |event, window, cx| {
                        if event.dragging() {
                            drag_owner.update(cx, |this, cx| {
                                if this.resizing.is_none() && this.selection.is_some() {
                                    this.select_cell(row_ix, ix, true, false, window, cx);
                                }
                            });
                        }
                    })
                    .when(kind == CellKind::Null, |d| {
                        d.text_color(cx.omarchy().secondary).italic()
                    })
                    .when(kind == CellKind::Empty, |d| {
                        d.text_color(cx.omarchy().accent).italic()
                    })
                    .when(matches!(kind, CellKind::Nan | CellKind::Infinity), |d| {
                        d.text_color(cx.omarchy().danger)
                    })
                    .child(div().truncate().child(value)),
            );
        }
        result
            .child(
                div()
                    .w(px(self.columns_width(visible.end..self.columns.len())))
                    .flex_shrink_0(),
            )
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        let Some(page) = &self.page else {
            return div();
        };
        let visible = self.visible_columns();
        let mut header = div()
            .flex()
            .h(px(if self.settings.show_plots {
                244.0
            } else {
                104.0
            }))
            .w_full()
            .flex_shrink_0()
            .bg(cx.omarchy().surface.opacity(0.2))
            .border_b_1()
            .border_color(cx.omarchy().border)
            .child(
                div()
                    .w(px(ROW_NUMBER_WIDTH))
                    .h_full()
                    .flex_shrink_0()
                    .px_3()
                    .pt_4()
                    .text_xs()
                    .text_color(cx.omarchy().secondary)
                    .bg(cx.omarchy().surface)
                    .border_r_1()
                    .border_color(cx.omarchy().border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_lg().child("#"))
                    .child("Source row"),
            )
            .child(
                div()
                    .w(px(self.columns_width(0..visible.start)))
                    .flex_shrink_0(),
            );
        for ix in visible.clone() {
            let column = self.columns[ix];
            let profile = &page.profiles[column];
            let arrow = match self.sort {
                Some((current, false)) if current == column => " ↑",
                Some((current, true)) if current == column => " ↓",
                _ => "",
            };
            header = header.child(
                div()
                    .id(("column", column))
                    .test_support()
                    .w(px(self.column_width(column)))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_r_1()
                    .border_color(cx.omarchy().border)
                    .relative()
                    .child(div().flex().items_center().gap_1()
                        .child(control(("sort", column), format!("{}{arrow}", profile.name), cx)
                            .flex_1().min_w_0().overflow_hidden()
                            .border_color(cx.omarchy().foreground.opacity(0.0))
                            .disabled(self.loading || !profile.sortable)
                            .on_click(cx.listener(move |this, _, window, cx| this.sort_column(column, window, cx))))
                        .child(self.column_menu(column,cx)))
                    .child(
                        div()
                            .text_xs()
                            .flex_shrink_0()
                            .text_color(cx.omarchy().accent)
                            .truncate()
                            .child(profile.dtype.clone()),
                    )
                    .when(self.settings.show_plots, |d| {
                        if profile.summary_limited {
                            d.child(
                                div()
                                    .h(px(128.0))
                                    .flex()
                                    .items_center()
                                    .text_xs()
                                    .text_color(cx.omarchy().secondary)
                                    .child("Summary skipped for large values"),
                            )
                        } else if matches!(profile.distribution, Distribution::None) {
                            d.child(plot(&profile.distribution, cx))
                        } else {
                            d.child(
                                button(("plot", column), "", ButtonVariant::Secondary, cx)
                                    .disabled(self.loading)
                                    .accessibility_label(format!(
                                        "Expand {} distribution",
                                        profile.name
                                    ))
                                    .p_2()
                                    .w_full()
                                    .h(px(128.0))
                                    .justify_start()
                                    .border_color(cx.omarchy().foreground.opacity(0.0))
                                    .child(plot(&profile.distribution, cx).w_full())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_detail(column, window, cx)
                                    })),
                            )
                        }
                    })
                    .child(gpui_omarchy::with_tooltip(
                        div().id(("missing",column)).text_xs().text_color(cx.omarchy().secondary)
                            .child(format!("{} nulls ({:.1}%)",number(profile.null_count),profile.null_count as f64 * 100.0 / page.profile_rows.max(1) as f64))
                            .child(div().mt_1().h(px(3.0)).w_full().bg(cx.omarchy().accent.opacity(0.15))
                                .child(div().h_full().w(relative(profile.null_count as f32 / page.profile_rows.max(1) as f32)).bg(cx.omarchy().danger))),
                        format!("{} sampled source rows · {} nulls · {} empty strings · {} NaN/infinite values{}",number(page.profile_rows),number(profile.null_count),number(profile.empty_count),number(profile.non_finite_count),if profile.summary_limited {" · Value summary unavailable"}else{""})))
                    .child(div().id(("resize-column",column)).test_support().absolute().top_0().right_0().w(px(7.0)).h_full()
                        .cursor(CursorStyle::ResizeLeftRight).hover(|d|d.bg(cx.omarchy().accent.opacity(0.5)))
                        .on_mouse_down(MouseButton::Left,cx.listener(move |this,event: &MouseDownEvent,_,cx| {
                            cx.stop_propagation();
                            if event.click_count==2 { this.resizing=None;this.fit_column(column,cx); }
                            else { this.resizing=Some((column,f32::from(event.position.x),this.column_width(column))); }
                        }))),
            );
        }
        header.child(
            div()
                .w(px(self.columns_width(visible.end..self.columns.len())))
                .flex_shrink_0(),
        )
    }

    fn table(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let viewport_owner = cx.weak_entity();
        let width = ROW_NUMBER_WIDTH + self.columns_width(0..self.columns.len());
        let header = self.header(cx);
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .border_1()
            .border_color(cx.omarchy().border)
            .rounded_lg()
            .child(
                div()
                    .id("horizontal-table")
                    .test_support()
                    .key_context("DataGrid")
                    .track_focus(&self.grid_focus)
                    .size_full()
                    .overflow_x_scroll()
                    // Ordinary wheel input belongs to rows, never to horizontal overflow.
                    .restrict_scroll_to_axis()
                    .track_scroll(&self.horizontal)
                    .on_prepaint(move |bounds, _, cx| {
                        // Use the actual table viewport; headers and cells share this value.
                        cx.defer(move |cx| {
                            let _ = viewport_owner.update(cx, |this, cx| {
                                let offset = this.horizontal.offset().x;
                                let row = this.row_position();
                                let moved_row = this
                                    .page
                                    .as_ref()
                                    .and_then(|page| this.settings.file(&page.path))
                                    .is_some_and(|file| file.row != row);
                                if moved_row {
                                    this.schedule_save(cx);
                                }
                                if this.viewport_width != bounds.size.width
                                    || this.viewport_x != offset
                                {
                                    this.viewport_width = bounds.size.width;
                                    this.viewport_x = offset;
                                    this.schedule_save(cx);
                                    cx.notify();
                                }
                            });
                        });
                    })
                    .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                        this.schedule_save(cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(width))
                            .min_w_full()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(
                                uniform_list("rows", self.rows.len(), move |range, _, cx| {
                                    let this = view.read(cx);
                                    range
                                        .map(|row| this.row(row, cx, view.clone()))
                                        .collect::<Vec<_>>()
                                })
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .map(|mut rows| {
                                    rows.style().restrict_scroll_to_axis = Some(true);
                                    rows
                                })
                                .track_scroll(&self.vertical),
                            )
                            .when(self.rows.is_empty(), |d| {
                                d.child(div().p_8().text_color(cx.omarchy().secondary).child(
                                    if self.filters.is_empty() {
                                        "This file has no rows."
                                    } else {
                                        "No rows match these filters."
                                    },
                                ))
                            }),
                    ),
            )
            .child(scrollbar(
                "horizontal-scrollbar",
                ScrollbarAxis::Horizontal,
                &self.horizontal,
                window,
                cx,
            ))
            .child(scrollbar(
                "vertical-scrollbar",
                ScrollbarAxis::Vertical,
                &self.vertical,
                window,
                cx,
            ))
    }

    fn toggle_plots(&mut self, cx: &mut Context<Self>) {
        self.settings.show_plots = !self.settings.show_plots;
        self.save_settings(cx);
        cx.notify();
    }

    fn open_detail(&mut self, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = &self.page else {
            return;
        };
        match page.column_detail(column) {
            Ok(detail) => {
                self.detail = Some(detail);
                self.panel_scroll.set_offset(point(px(0.0), px(0.0)));
                self.detail_scope = None;
                self.focus.focus(window, cx);
            }
            Err(error) => self.error = Some(format!("Cannot summarize this column: {error:#}")),
        }
        cx.notify();
    }

    fn close_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from_overview = self.detail.is_some() && self.detail_scope.is_some();
        if self.panel.take().is_some() || self.detail.take().is_some() {
            if from_overview && self.overview.is_some() {
                self.panel = Some(Panel::Overview);
            }
            self.detail = None;
            self.detail_scope = None;
            if self.panel.is_none() && self.selection.is_some() {
                self.grid_focus.focus(window, cx);
            } else {
                self.focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    fn request_association(&mut self, cx: &mut Context<Self>) {
        if self.association_busy {
            return;
        }
        self.association_busy = true;
        self.notice = None;
        self.association_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async { association::associate() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.association_busy = false;
                match result {
                    Ok(message) => {
                        this.notice = Some(message);
                        this.association_offer = false;
                        this.settings.association_prompted = true;
                        this.save_settings(cx);
                    }
                    Err(error) => this.error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn app_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        menu(
            "app-menu-popover",
            button("app-menu", "", ButtonVariant::Secondary, cx)
                .child(gpui_omarchy::icon(gpui_omarchy::IconName::Menu))
                .aria_label("File and view menu"),
            vec![
                MenuItem::new("Open file…").action(OpenFile),
                MenuItem::new("Show column plots")
                    .checked(self.settings.show_plots)
                    .separator_before(),
                MenuItem::new("Associate Parquet files…").separator_before(),
                MenuItem::new("Find a column…")
                    .action(FindColumn)
                    .disabled(self.page.is_none()),
                MenuItem::new("Show / hide columns…").disabled(self.page.is_none() || self.loading),
                MenuItem::new("File details…").disabled(self.page.is_none()),
                MenuItem::new("File overview…").disabled(self.page.is_none() || self.loading),
                MenuItem::new("Recent files…").separator_before(),
                MenuItem::new("Open remote Parquet…"),
                MenuItem::new("CSV / TSV options…"),
                MenuItem::new("Keyboard shortcuts…")
                    .action(ShowShortcuts)
                    .separator_before(),
                MenuItem::new("Quit").action(Quit).separator_before(),
            ],
            move |index, window, cx| {
                owner.update(cx, |this, cx| match index {
                    0 => this.open_file(&OpenFile, window, cx),
                    1 => this.toggle_plots(cx),
                    2 => {
                        this.association_offer = true;
                        this.notice = None;
                        cx.notify();
                    }
                    3 => this.find_column(&FindColumn, window, cx),
                    4 => this.show_panel(Panel::Columns, window, cx),
                    5 => this.show_info(window, cx),
                    6 => this.start_overview(window, cx),
                    7 => this.show_panel(Panel::Recent, window, cx),
                    8 => this.show_panel(Panel::Url, window, cx),
                    9 => this.show_csv_options(window, cx),
                    10 => this.show_shortcuts(&ShowShortcuts, window, cx),
                    11 => cx.quit(),
                    _ => {}
                })
            },
        )
    }

    fn show_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.detail = None;
        self.detail_scope = None;
        self.panel_scroll.set_offset(point(px(0.0), px(0.0)));
        self.panel = Some(panel);
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn find_column(&mut self, _: &FindColumn, window: &mut Window, cx: &mut Context<Self>) {
        if self.page.is_none() {
            return;
        }
        self.show_panel(Panel::Find, window, cx);
        self.search_input
            .update(cx, |input, cx| input.focus(window, cx));
    }

    fn jump_to_column(&mut self, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.columns.contains(&column) {
            self.hidden_columns
                .retain(|name| name != &self.page.as_ref().unwrap().profiles[column].name);
            self.refresh_columns();
        }
        let position = self.columns.iter().position(|&c| c == column).unwrap_or(0);
        let max_x = (ROW_NUMBER_WIDTH + self.columns_width(0..self.columns.len())
            - f32::from(self.viewport_width))
        .max(0.0);
        self.viewport_x = -px((ROW_NUMBER_WIDTH + self.columns_width(0..position)).min(max_x));
        self.horizontal.set_offset(point(self.viewport_x, px(0.0)));
        self.panel = None;
        self.focus.focus(window, cx);
        self.save_settings(cx);
        cx.notify();
    }

    fn inspect_cell(
        &mut self,
        row: usize,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = &self.page else {
            return;
        };
        match page.inspect_cell(row, column) {
            Ok(detail) => {
                self.text_area.update(cx, |input, cx| {
                    input.set_value(detail.text.clone(), window, cx)
                });
                self.show_panel(Panel::Cell(detail), window, cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn show_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = &self.page else {
            return;
        };
        let info = &page.source.info;
        let mut text = format!(
            "{}\n\nFormat: {}\nRows: {}\nColumns: {}\nFile size: {}\nCompression: {}\nEncodings: {}\nWriter: {}\n\nSchema (file order{}):\n",
            page.source.location.display(),
            info.format,
            number(page.total_rows),
            number(page.profiles.len()),
            bytes(page.file_bytes),
            if info.codecs.is_empty() {
                "Not applicable".into()
            } else {
                info.codecs.join(", ")
            },
            if info.encodings.is_empty() {
                "Not recorded".into()
            } else {
                info.encodings.join(", ")
            },
            info.created_by.as_deref().unwrap_or("Not recorded"),
            if info.inferred_types {
                "; types inferred from the first 1,000 records"
            } else {
                ""
            }
        );
        use std::fmt::Write;
        for profile in &page.profiles {
            let _ = writeln!(text, "{}: {}", profile.name, profile.dtype);
            if text.len() > 65_536 {
                text.push_str("… Further schema entries omitted\n");
                break;
            }
        }
        if !info.groups.is_empty() {
            let _ = writeln!(
                text,
                "\n{} {}:",
                number(info.groups.len()),
                if info.format == "Parquet" {
                    "row groups"
                } else {
                    "record batches"
                }
            );
            for (ix, group) in info.groups.iter().enumerate() {
                let _ = writeln!(
                    text,
                    "{}: {} rows · {} stored{}",
                    ix + 1,
                    number(group.rows),
                    bytes(group.compressed),
                    if group.uncompressed > 0 {
                        format!(" · {} uncompressed", bytes(group.uncompressed))
                    } else {
                        String::new()
                    }
                );
                if text.len() > 65_536 {
                    text.push_str("… Further groups omitted\n");
                    break;
                }
            }
        }
        self.text_area
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.show_panel(Panel::Info, window, cx);
    }

    fn show_csv_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.csv_target = self.attempted_source.clone().filter(|location| matches!(location, Location::Local(path) if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv") || e.eq_ignore_ascii_case("tsv"))));
        self.csv_draft = self
            .csv_target
            .as_ref()
            .and_then(|l| self.settings.file_location(l))
            .and_then(|f| f.csv_options)
            .unwrap_or(self.settings.csv_options);
        self.show_panel(Panel::Csv, window, cx);
    }

    fn start_overview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some(page) = &self.page else {
            return;
        };
        let source = Arc::clone(&page.source);
        self.operation = Cancellation::default();
        let cancel = self.operation.clone();
        self.loading = true;
        self.notice = Some("Building a bounded file overview…".into());
        cx.notify();
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { data::overview(source, &cancel) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.loading = false;
                this.notice = None;
                match result {
                    Ok(overview) => {
                        this.overview = Some(Arc::new(overview));
                        this.show_panel(Panel::Overview, window, cx);
                    }
                    Err(error) => this.error = Some(format!("Cannot summarize source: {error:#}")),
                }
                if let Some(location) = this.pending_source.take() {
                    this.open_source(location, window, cx);
                }
                cx.notify();
            });
        }));
    }

    fn panel_view(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let panel = self.panel.as_ref().unwrap();
        let title = match panel {
            Panel::Find => "Find a column",
            Panel::Cell(_) => "Inspect cell",
            Panel::Info => "File details",
            Panel::Recent => "Recent files",
            Panel::Url => "Open remote Parquet",
            Panel::Csv => "CSV / TSV options",
            Panel::Overview => "File overview",
            Panel::Columns => "Visible columns",
            Panel::Filter(_) => "Filter column",
            Panel::Shortcuts => "Keyboard shortcuts",
        };
        let mut root =
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(control("close-panel", "← Back", cx).on_click(
                            cx.listener(|this, _, window, cx| this.close_detail(window, cx)),
                        ))
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        ),
                )
                .when_some(self.error.clone(), |d, error| {
                    d.child(div().text_color(cx.omarchy().danger).child(error))
                })
                .when_some(self.notice.clone(), |d, notice| {
                    d.child(div().text_color(cx.omarchy().secondary).child(notice))
                });
        match panel {
            Panel::Shortcuts => {
                root = root.child(self.shortcuts_panel(window, cx));
            }
            Panel::Columns => {
                root = root.child(self.columns_panel(window, cx));
            }
            Panel::Filter(column) => {
                root = root.child(self.filter_panel(*column, window, cx));
            }
            Panel::Find => {
                let page = self.page.as_ref().unwrap();
                let query = self.search_input.read(cx).value().trim().to_lowercase();
                let matches: Vec<_> = page
                    .profiles
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        p.name.to_lowercase().contains(&query)
                            || p.dtype.to_lowercase().contains(&query)
                    })
                    .map(|(index, _)| index)
                    .collect();
                let owner = cx.entity();
                root = root
                    .child(input("column-search", &self.search_input, window, cx).w_full())
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.omarchy().secondary)
                            .child(format!(
                                "{} matches · Select a column to jump to it",
                                number(matches.len())
                            )),
                    )
                    .child(
                        uniform_list("column-results", matches.len(), move |range, _, cx| {
                            let viewer = owner.read(cx);
                            let page = viewer.page.as_ref().unwrap();
                            range
                                .map(|row| {
                                    let column = matches[row];
                                    let profile = &page.profiles[column];
                                    let target = owner.clone();
                                    control(
                                        ("find-column", column),
                                        format!("{}   ·   {}", profile.name, profile.dtype),
                                        cx,
                                    )
                                    .h(px(40.0))
                                    .w_full()
                                    .justify_start()
                                    .on_click(move |_, window, cx| {
                                        target.update(cx, |this, cx| {
                                            this.jump_to_column(column, window, cx)
                                        })
                                    })
                                    .into_any_element()
                                })
                                .collect()
                        })
                        .track_scroll(&self.find_scroll)
                        .flex_1()
                        .min_h_0(),
                    );
            }
            Panel::Cell(detail) => {
                root=root.child(div().flex().gap_4().items_center()
                    .child(div().text_lg().child(detail.name.clone()))
                    .child(div().text_color(cx.omarchy().accent).child(detail.dtype.clone()))
                    .child(div().child(format!("Source row {}{}",number(detail.source_row),if detail.null { " · null" } else { "" })))
                    .child(div().flex_1())
                    .child(control("copy-cell",if detail.truncated { "Copy displayed text" } else { "Copy value" },cx).on_click(cx.listener(|this,_,_,cx| {
                        if let Some(Panel::Cell(detail))=&this.panel { cx.write_to_clipboard(ClipboardItem::new_string(detail.text.clone())); this.notice=Some("Copied to clipboard".into()); cx.notify(); }
                    }))))
                    .when(detail.truncated,|d|d.child(div().text_sm().text_color(cx.omarchy().secondary).child("Showing the first 65,536 characters to keep the viewer responsive. Copy includes only the displayed text.")))
                    .child(gpui_omarchy::textarea("cell-value",&self.text_area,window,cx).flex_1().min_h_0());
            }
            Panel::Info => {
                root = root.child(
                    gpui_omarchy::textarea("file-details", &self.text_area, window, cx)
                        .flex_1()
                        .min_h_0(),
                );
            }
            Panel::Recent => {
                root = root.child(
                    control("clear-history", "Clear history", cx)
                        .disabled(self.settings.files.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.files.clear();
                            this.history_suppressed = true;
                            this.save_settings(cx);
                            this.notice = Some("History cleared".into());
                            cx.notify();
                        })),
                );
                let mut list = div()
                    .id("recent-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.panel_scroll)
                    .flex_1()
                    .min_h_0();
                for (index, file) in self.settings.files.iter().enumerate() {
                    let location = file
                        .location
                        .clone()
                        .unwrap_or_else(|| Location::Local(file.path.clone()));
                    let label = format!("{}   ·   {}", location.name(), location.display());
                    list = list.child(
                        control(("recent-file", index), label, cx)
                            .w_full()
                            .justify_start()
                            .disabled(self.loading)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_source(location.clone(), window, cx)
                            })),
                    );
                }
                if self.settings.files.is_empty() {
                    list = list.child("No recent files");
                }
                root = root.child(list);
            }
            Panel::Url => {
                root=root.child(div().text_sm().child(if cfg!(feature="cloud") { "Open one Parquet object from S3, Azure, GCS, or HTTP(S). Only the footer and required column ranges are fetched." } else { "Remote sources are available in the cloud-enabled download." }))
                    .child(input("source-url",&self.url_input,window,cx).w_full())
                    .child(div().text_sm().child("AWS shared credential profile (optional)"))
                    .child(input("aws-profile",&self.profile_input,window,cx).w(px(300.0)))
                    .child(div().text_sm().text_color(cx.omarchy().secondary).child("Uses your existing provider credentials. Signed URLs stay out of history. Each request has a 30-second timeout; loading can be cancelled."))
                    .child(control("open-url","Open",cx).disabled(!cfg!(feature="cloud") || self.loading).on_click(cx.listener(|this,_,window,cx| {
                        match Location::remote(&this.url_input.read(cx).value()) {
                            Ok(location) => { this.settings.aws_profile=this.profile_input.read(cx).value().trim().chars().take(128).collect(); this.save_settings(cx); this.open_source(location,window,cx); }
                            Err(error) => { this.error=Some(error.to_string()); cx.notify(); }
                        }
                    })));
            }
            Panel::Csv => {
                root = root.child(self.csv_controls(cx));
            }
            Panel::Overview => {
                let overview = self.overview.as_ref().unwrap();
                let scope = overview_scope(overview);
                root = root
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.omarchy().secondary)
                            .child(scope),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .px_3()
                            .text_xs()
                            .text_color(cx.omarchy().secondary)
                            .child(div().w(px(240.0)).child("Column / type"))
                            .child(div().w(px(120.0)).child("Nulls"))
                            .child(div().w(px(120.0)).child("Distinct non-null"))
                            .child(div().flex_1().child("Range / top value")),
                    );
                let owner = cx.entity();
                root = root.child(
                    uniform_list(
                        "overview-columns",
                        overview.details.len(),
                        move |range, _, cx| {
                            let viewer = owner.read(cx);
                            let overview = viewer.overview.as_ref().unwrap();
                            range
                                .map(|column| {
                                    let detail = &overview.details[column];
                                    let profile = &detail.profile;
                                    let range = overview_range(profile);
                                    let target = owner.clone();
                                    control(("overview-column", column), "", cx)
                                        .h(px(44.0))
                                        .w_full()
                                        .justify_start()
                                        .child(
                                            div().w(px(240.0)).truncate().child(format!(
                                                "{} · {}",
                                                profile.name, profile.dtype
                                            )),
                                        )
                                        .child(div().w(px(120.0)).child(format!(
                                            "{} ({:.1}%)",
                                            number(profile.null_count),
                                            profile.null_count as f64 * 100.0
                                                / detail.rows.max(1) as f64
                                        )))
                                        .child(div().w(px(120.0)).child(
                                            if detail.distinct_available {
                                                number(detail.distinct)
                                            } else {
                                                "Unavailable".into()
                                            },
                                        ))
                                        .child(div().flex_1().truncate().child(range))
                                        .on_click(move |_, window, cx| {
                                            target.update(cx, |this, cx| {
                                                let overview = this.overview.as_ref().unwrap();
                                                this.panel_scroll
                                                    .set_offset(point(px(0.0), px(0.0)));
                                                this.detail_scope = Some(overview_scope(overview));
                                                this.detail =
                                                    Some(overview.details[column].clone());
                                                this.panel = None;
                                                this.focus.focus(window, cx);
                                                cx.notify();
                                            })
                                        })
                                        .into_any_element()
                                })
                                .collect()
                        },
                    )
                    .track_scroll(&self.find_scroll)
                    .flex_1()
                    .min_h_0(),
                );
            }
        }
        root
    }

    fn csv_controls(&self, cx: &mut Context<Self>) -> Div {
        let target = self.csv_target.as_ref().map_or_else(
            || "Defaults for the next CSV/TSV file".into(),
            Location::display,
        );
        let delimiters = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child("Delimiter")
            .children(
                [
                    Delimiter::Auto,
                    Delimiter::Comma,
                    Delimiter::Tab,
                    Delimiter::Semicolon,
                    Delimiter::Pipe,
                ]
                .into_iter()
                .enumerate()
                .map(|(index, delimiter)| {
                    control(("delimiter", index), delimiter.label(), cx)
                        .selected(self.csv_draft.delimiter == delimiter)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.csv_draft.delimiter = delimiter;
                            cx.notify();
                        }))
                }),
            );
        let header = control(
            "csv-header",
            if self.csv_draft.has_header {
                "✓ First record contains column names"
            } else {
                "First record contains data"
            },
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.csv_draft.has_header = !this.csv_draft.has_header;
            cx.notify();
        }));
        let encoding = control(
            "csv-encoding",
            if self.csv_draft.lossy_utf8 {
                "UTF-8 · replace invalid bytes"
            } else {
                "UTF-8 · report invalid bytes"
            },
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.csv_draft.lossy_utf8 = !this.csv_draft.lossy_utf8;
            cx.notify();
        }));
        let types = control(
            "csv-types",
            if self.csv_draft.infer_types {
                "Infer types from first 1,000 records"
            } else {
                "Read all columns as text"
            },
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.csv_draft.infer_types = !this.csv_draft.infer_types;
            cx.notify();
        }));
        let apply = control("apply-csv", "Save and apply", cx)
            .disabled(self.loading)
            .on_click(cx.listener(|this, _, window, cx| {
                this.settings.csv_options = this.csv_draft;
                this.save_settings(cx);
                if let Some(location) = this.csv_target.clone() {
                    this.csv_override = Some(this.csv_draft);
                    this.load_location(location, 0, true, window, cx);
                } else {
                    this.close_detail(window, cx);
                }
            }));
        div().flex().flex_col().gap_4().child(div().text_sm().child(target)).child(delimiters)
            .child(header).child(encoding).child(types)
            .child(div().text_sm().text_color(cx.omarchy().secondary).child("CSV/TSV requires one scan to count records and build a disk-backed row index. Subsequent pages use that index. Types inferred from a sample may not describe later records."))
            .child(apply)
    }

    fn footer(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let Some(page) = &self.page else {
            return div();
        };
        let current = page.page_number();
        let last = page.page_count();
        let owner = cx.entity();
        let state = PaginationState::new(current, last)
            .disabled(self.loading)
            .on_change(move |page, window, cx| {
                owner.update(cx, |this, cx| this.navigate(page, window, cx))
            });
        let range = if let Some(matches) = page.matching_rows {
            format!(
                "{}–{} of {} matching rows · {} source rows",
                if matches == 0 { 0 } else { page.offset + 1 },
                page.offset + page.frame.height(),
                number(matches),
                number(page.total_rows)
            )
        } else if page.total_rows == 0 {
            "0 rows".into()
        } else {
            format!(
                "Rows {}–{} of {}",
                number(page.offset + 1),
                number(page.offset + page.frame.height()),
                number(page.total_rows)
            )
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pt_3()
            .flex_shrink_0()
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .text_color(cx.omarchy().secondary)
                    .child(format!(
                        "{range}  ·  {} columns  ·  {}  ·  {}",
                        if self.columns.len() == page.profiles.len() {
                            number(page.profiles.len())
                        } else {
                            format!("{} / {}", self.columns.len(), page.profiles.len())
                        },
                        bytes(page.file_bytes),
                        page.source.info.format
                    ))
                    .child(if self.loading {
                        if self.scanning {
                            format!(
                                "Filtering: {} / {} rows scanned · Total matches pending",
                                number(self.scan_progress.load(Ordering::Acquire)),
                                number(self.scan_total.load(Ordering::Acquire))
                            )
                        } else {
                            "Working…".into()
                        }
                    } else {
                        format!(
                            "Read only  ·  Plots: {} source rows  ·  Sort: loaded rows{}",
                            number(page.profile_rows),
                            if page.source.info.inferred_types {
                                " · Inferred types"
                            } else {
                                ""
                            }
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_color(cx.omarchy().secondary)
                                    .child("Rows / page"),
                            )
                            .children(
                                PAGE_SIZES
                                    .into_iter()
                                    .map(RowLimit::Paged)
                                    .chain(std::iter::once(RowLimit::All))
                                    .enumerate()
                                    .map(|(ix, limit)| {
                                        control(
                                            ("page-size", ix),
                                            match limit {
                                                RowLimit::Paged(size) => size.to_string(),
                                                RowLimit::All => "All".into(),
                                            },
                                            cx,
                                        )
                                        .selected(limit == self.settings.row_limit)
                                        .disabled(self.loading)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.set_row_limit(limit, window, cx)
                                            }),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                control("previous", "‹", cx)
                                    .aria_label("Previous page")
                                    .disabled(self.loading || current == 1)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.previous(&PreviousPage, window, cx)
                                    })),
                            )
                            .child(pagination("pagination", state, cx))
                            .child(
                                control("next", "›", cx)
                                    .aria_label("Next page")
                                    .disabled(self.loading || current == last)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.next(&NextPage, window, cx)
                                    })),
                            )
                            .child(input("page-input", &self.page_input, window, cx).w(px(62.0)))
                            .child(
                                div()
                                    .text_color(cx.omarchy().secondary)
                                    .child(format!("/ {}", number(last))),
                            )
                            .child(control("go", "Go", cx).disabled(self.loading).on_click(
                                cx.listener(|this, _, window, cx| this.jump(window, cx)),
                            )),
                    ),
            )
    }

    fn detail_view(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        use gpui_omarchy::chart::{BarAlignment, BarChart};
        let detail = self.detail.as_ref().unwrap();
        let page = self.page.as_ref().unwrap();
        let horizontal = matches!(detail.profile.distribution, Distribution::Categories(_));
        let bars = detail.bars.iter().cloned().enumerate().collect::<Vec<_>>();
        let total = detail.rows.max(1);
        let chart = BarChart::new(bars)
            .id("detailed-distribution")
            .band(|(index, bar): &(usize, data::DetailBar)| PlotBand {
                index: *index,
                label: bar.axis.clone(),
            })
            .value(|(_, bar)| bar.count as f64)
            .alignment(if horizontal {
                BarAlignment::Left
            } else {
                BarAlignment::Bottom
            })
            .padding_inner(if horizontal { 0.4 } else { 0.06 })
            .max_band_width(px(if horizontal { 24.0 } else { 1000.0 }))
            .value_axis(true)
            .band_tick_count(if horizontal { detail.bars.len() } else { 8 })
            .value_tick_format(|count| number(count.round().max(0.0) as usize))
            .name("Rows")
            .tooltip_title(|(_, bar)| bar.label.clone().into())
            .tooltip_value(move |(_, bar), _| {
                format!(
                    "{} ({:.1}% of sample)",
                    number(bar.count),
                    bar.count as f64 * 100.0 / total as f64
                )
                .into()
            });
        let mut stats = vec![
            ("Sample rows", number(detail.rows)),
            ("Non-null", number(detail.rows - detail.profile.null_count)),
            (
                "Nulls",
                format!(
                    "{} ({:.1}%)",
                    number(detail.profile.null_count),
                    detail.profile.null_count as f64 * 100.0 / total as f64
                ),
            ),
            (
                "Distinct non-null",
                if detail.distinct_available {
                    number(detail.distinct)
                } else {
                    "Unavailable".into()
                },
            ),
            ("Non-finite", number(detail.profile.non_finite_count)),
        ]
        .into_iter()
        .map(|(label, value)| (label.to_string(), value))
        .collect::<Vec<_>>();
        stats.extend(detail.statistics.clone());
        let scope = self.detail_scope.clone().unwrap_or_else(|| {
            if let Some(matches)=page.matching_rows {
                return format!("{} · First {} matching rows on this page · {} total matches · {} source rows · {}",page.source.location.name(),number(detail.rows),number(matches),number(page.total_rows),if detail.rows==matches {"All matching rows"} else {"Sample only"});
            }
            format!(
                "{} · Source rows {}–{} of {} · {}",
                page.source.location.name(),
                number(if detail.rows == 0 {
                    0
                } else {
                    page.source_row(0) + 1
                }),
                number(if detail.rows == 0 {
                    0
                } else {
                    page.source_row(detail.rows - 1) + 1
                }),
                number(page.total_rows),
                if page.matching_rows.is_some() {
                    "Matching rows on this page; sample only"
                } else if detail.rows == page.total_rows && page.offset == 0 {
                    "All file rows"
                } else if detail.rows < page.frame.height() {
                    "First 1,000 loaded rows; sample only"
                } else {
                    "Current page; sample only"
                }
            )
        });
        div().flex_1().min_h_0().flex().flex_col().gap_4()
            .child(div().flex().items_center().gap_4()
                .child(control("close-chart", if self.detail_scope.is_some() { "← Back to overview" } else { "← Back to table" }, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.close_detail(window, cx))))
                .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(detail.profile.name.clone()))
                .child(div().text_color(cx.omarchy().accent).child(detail.profile.dtype.clone())))
            .child(div().text_sm().text_color(cx.omarchy().secondary).child(scope))
            .child(div().flex().flex_1().min_h_0().gap_4()
                .child(div().id("chart-detail").test_support().flex_1().min_w_0().min_h_0().p_4()
                    .border_1().border_color(cx.omarchy().border).child(chart))
                .child(div().relative().w(px(260.0)).flex_shrink_0().h_full()
                    .child(div().id("column-statistics").test_support().size_full().overflow_y_scroll().track_scroll(&self.panel_scroll)
                        .flex().flex_col().gap_2().pr_3().children(stats.into_iter().map(|(label,value)| {
                            div().w_full().flex_shrink_0().p_3().border_1().border_color(cx.omarchy().border)
                                .child(div().text_xs().text_color(cx.omarchy().secondary).child(label))
                                .child(div().text_lg().child(value))
                        })))
                    .child(scrollbar("statistics-scrollbar",ScrollbarAxis::Vertical,&self.panel_scroll,window,cx))))
            .child(div().text_sm().text_color(cx.omarchy().secondary).child(match &detail.profile.distribution {
                Distribution::Histogram { bins, min, max, temporal } => format!("{} · {} equal-width bins · Minimum: {min} · Maximum: {max} · Approximate bin starts; hover for exact observed values and counts", if *temporal { "Time distribution" } else { "Distribution" }, bins.len()),
                Distribution::Categories(_) if detail.profile.dtype == "bool" => "Boolean counts · False and true · Hover for counts and percentages".into(),
                Distribution::Categories(_) => "Category counts · Up to 20 most frequent values; remaining values grouped as Other · Hover for counts and percentages".into(),
                Distribution::None => "No plottable values".into(),
            }))
            .child(div().text_xs().text_color(cx.omarchy().secondary).child("Esc to return · Numeric summaries exclude null/NaN/infinity and use floating-point arithmetic; quartiles interpolate · String lengths count Unicode characters"))
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_file = self.page.is_some();
        let title = self
            .page
            .as_ref()
            .map_or("Parquet Viewer".to_owned(), |p| p.source.location.name());
        let root = div()
            .id("viewer")
            .key_context(
                if has_file && self.panel.is_none() && self.detail.is_none() && !self.loading {
                    "ParquetViewer TableView"
                } else {
                    "ParquetViewer"
                },
            )
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::find_column))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::copy_rows))
            .on_action(cx.listener(Self::inspect_selection))
            .on_action(cx.listener(Self::table_navigation))
            .on_action(cx.listener(Self::select_all_cells))
            .on_action(cx.listener(Self::focus_page))
            .on_action(cx.listener(Self::show_shortcuts))
            .on_action(cx.listener(|this, _: &MoveLeft, window, cx| {
                this.move_selection(-1, 0, false, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveRight, window, cx| {
                this.move_selection(1, 0, false, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveUp, window, cx| {
                this.move_selection(0, -1, false, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveDown, window, cx| {
                this.move_selection(0, 1, false, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendLeft, window, cx| {
                this.move_selection(-1, 0, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendRight, window, cx| {
                this.move_selection(1, 0, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendUp, window, cx| {
                this.move_selection(0, -1, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtendDown, window, cx| {
                this.move_selection(0, 1, true, window, cx)
            }))
            .on_mouse_move(
                cx.listener(|this, event: &MouseMoveEvent, _, cx| this.resize_move(event, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing.take().is_some() {
                        this.save_settings(cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &CloseDetail, window, cx| this.escape_view(window, cx)),
            )
            .on_action(|_: &Quit, _, cx| cx.quit())
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                if let Some(path) = paths.paths().first() {
                    this.open_path(path.clone(), window, cx);
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .p_5()
            .font_family(cx.omarchy().font.clone())
            .bg(cx.omarchy().background)
            .text_color(cx.omarchy().foreground);
        if self.panel.is_some() {
            return root.child(self.panel_view(window, cx)).into_any_element();
        }
        if self.detail.is_some() {
            return root.child(self.detail_view(window, cx)).into_any_element();
        }
        let toolbar = div()
            .flex()
            .items_center()
            .gap_3()
            .pb_3()
            .child(self.app_menu(cx))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(title),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.omarchy().secondary)
                    .child(if has_file {
                        "Click to select · Enter to inspect · F1 for shortcuts"
                    } else {
                        "A small window into your data"
                    }),
            );
        root.child(toolbar.when(self.loading, |d| {
            d.child(
                control("cancel-load", "Cancel", cx).on_click(cx.listener(|this, _, _, cx| {
                    this.operation.cancel();
                    this.notice = Some("Cancelling…".into());
                    cx.notify();
                })),
            )
        }))
        .when(self.association_offer, |d| {
            d.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .mb_3()
                    .border_1()
                    .border_color(cx.omarchy().border)
                    .bg(cx.omarchy().surface)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(250.0))
                            .text_sm()
                            .child("Open .parquet files in Parquet Viewer by default?"),
                    )
                    .child(
                        control(
                            "associate",
                            if cfg!(target_os = "windows") {
                                "Choose in Windows Settings"
                            } else {
                                "Make default"
                            },
                            cx,
                        )
                        .disabled(self.association_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.request_association(cx))),
                    )
                    .child(
                        control("association-decline", "No thanks", cx)
                            .disabled(self.association_busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.association_offer = false;
                                this.settings.association_prompted = true;
                                this.save_settings(cx);
                                cx.notify();
                            })),
                    ),
            )
        })
        .when_some(self.notice.clone(), |d, message| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .mb_3()
                    .bg(cx.omarchy().surface)
                    .text_sm()
                    .child(message)
                    .child(
                        control("notice-dismiss", "Dismiss", cx).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            },
                        )),
                    ),
            )
        })
        .when_some(self.error.clone(), |d, error| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .mb_3()
                    .bg(cx.omarchy().danger.opacity(0.1))
                    .text_color(cx.omarchy().danger)
                    .text_sm()
                    .child(error)
                    .child(control("dismiss", "Dismiss", cx).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.error = None;
                            cx.notify();
                        },
                    ))),
            )
        })
        .when(has_file, |d| {
            d.child(self.filter_chips(cx))
                .child(self.table(window, cx))
                .child(self.footer(window, cx))
        })
        .when(!has_file, |d| {
            d.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("A small window into your data."),
                    )
                    .child(div().text_color(cx.omarchy().secondary).child(
                        "Open Parquet, IPC, CSV or TSV. Drop a file here to explore its columns.",
                    ))
                    .child(
                        button("open", "Open file…", ButtonVariant::Primary, cx)
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_file(&OpenFile, window, cx)
                            })),
                    )
                    .child(div().text_xs().text_color(cx.omarchy().secondary).child(
                        if self.loading {
                            if self.scanning {format!("Filtering saved view: {} / {} rows scanned · Total matches pending",number(self.scan_progress.load(Ordering::Acquire)),number(self.scan_total.load(Ordering::Acquire)))} else {"Reading the first page…".into()}
                        } else {
                            "Ctrl / Cmd+O to open · Large files are read one page at a time".into()
                        },
                    )),
            )
        })
        .into_any_element()
    }
}

fn control(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> gpui_omarchy::Button {
    button(id, label, ButtonVariant::Outline, cx).py_1()
}

fn plot(distribution: &Distribution, cx: &App) -> Div {
    let accent = cx.omarchy().accent;
    let muted = cx.omarchy().secondary;
    let base = div().flex().flex_col().h(px(112.0)).gap_1().text_xs();
    match distribution {
        Distribution::Histogram {
            bins,
            min,
            max,
            temporal,
        } => {
            let peak = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
            base.child(div().text_color(muted).child(if *temporal {
                "Time distribution"
            } else {
                "Distribution"
            }))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(2.0))
                    .h(px(66.0))
                    .w_full()
                    .children(bins.iter().map(|count| {
                        div()
                            .flex_1()
                            .h(px(*count as f32 / peak * 64.0))
                            .bg(accent.opacity(0.8))
                            .rounded_t_sm()
                    })),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .text_color(muted)
                    .child(div().max_w(px(92.0)).truncate().child(min.clone()))
                    .child(div().max_w(px(92.0)).truncate().child(max.clone())),
            )
        }
        Distribution::Categories(bars) => {
            let peak = bars.iter().map(|b| b.count).max().unwrap_or(1).max(1) as f32;
            base.children(bars.iter().map(|bar| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(19.0))
                    .child(
                        div()
                            .w(px(57.0))
                            .truncate()
                            .text_color(muted)
                            .child(bar.label.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h(px(6.0))
                            .bg(accent.opacity(0.08))
                            .rounded_sm()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(bar.count as f32 / peak))
                                    .rounded_sm()
                                    .bg(accent.opacity(0.8)),
                            ),
                    )
                    .child(
                        div()
                            .w(px(26.0))
                            .text_right()
                            .text_color(muted)
                            .child(bar.count.to_string()),
                    )
            }))
        }
        Distribution::None => base
            .justify_center()
            .text_color(muted)
            .child("No plottable values"),
    }
}

fn overview_range(profile: &data::ColumnProfile) -> String {
    match &profile.distribution {
        Distribution::Histogram { min, max, .. } => format!("{min} – {max}"),
        Distribution::Categories(bars) => {
            let top = if profile.dtype == "bool" {
                bars.iter().max_by_key(|bar| bar.count)
            } else {
                // String categories are ordered by frequency; their final Other
                // bar aggregates multiple values and must not become the top value.
                bars.first()
            };
            top.map(|bar| format!("{} ({} rows)", bar.label, number(bar.count)))
                .unwrap_or_else(|| "—".into())
        }
        Distribution::None => "—".into(),
    }
}

fn overview_scope(overview: &data::FileOverview) -> String {
    format!(
        "{} · {} of {} rows · {}",
        overview.page.source.location.name(),
        number(overview.page.frame.height()),
        number(overview.page.total_rows),
        if overview.page.frame.height() == overview.page.total_rows {
            "All source rows; exact counts".into()
        } else {
            format!(
                "{} evenly spaced windows; stratified preview, not exact file statistics",
                overview.windows
            )
        }
    )
}

fn number(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

fn bytes(value: u64) -> String {
    if value >= 1 << 30 {
        format!("{:.1} GiB", value as f64 / (1_u64 << 30) as f64)
    } else if value >= 1 << 20 {
        format!("{:.1} MiB", value as f64 / (1_u64 << 20) as f64)
    } else if value >= 1 << 10 {
        format!("{:.1} KiB", value as f64 / (1_u64 << 10) as f64)
    } else {
        format!("{value} B")
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod tests {
    use super::Viewer;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, ScrollDelta, TestAppContext, WindowBounds, WindowOptions, point, px,
        size,
    };
    use parquetviewer::data::load_page;
    use polars_core::prelude::{DataFrame, IntoColumn, NamedFrom, Series};
    use polars_io::prelude::ParquetWriter;
    use std::sync::Arc;

    #[test]
    fn overview_shows_the_most_common_boolean_and_an_actual_string_value() {
        let mut frame = DataFrame::new(
            12,
            vec![
                Series::new(
                    "active".into(),
                    [
                        false, false, true, true, true, true, true, true, true, true, true, true,
                    ],
                )
                .into_column(),
                Series::new(
                    "label".into(),
                    ["a", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"],
                )
                .into_column(),
            ],
        )
        .unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        ParquetWriter::new(file.as_file_mut())
            .finish(&mut frame)
            .unwrap();
        let page = load_page(file.path(), 0, 500).unwrap();
        assert_eq!(super::overview_range(&page.profiles[0]), "true (10 rows)");
        assert_eq!(super::overview_range(&page.profiles[1]), "a (2 rows)");
    }

    fn toggle_plots(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
        window.click("app-menu", cx);
        window.render_frame(cx);
        window.click(("menu-item", 1usize), cx);
        window.render_frame(cx);
    }

    pub(super) fn fixture(count: usize) -> tempfile::NamedTempFile {
        let columns = (0..40)
            .map(|i| {
                Series::new(
                    format!("column_{i}").into(),
                    (0..count as i64).rev().collect::<Vec<_>>(),
                )
                .into_column()
            })
            .collect();
        let mut frame = DataFrame::new(count, columns).unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        ParquetWriter::new(file.as_file_mut())
            .with_row_group_size(Some(73))
            .finish(&mut frame)
            .unwrap();
        file
    }

    #[gpui_kit::test]
    fn expands_column_details_without_changing_window_state(cx: &mut TestAppContext) {
        let file = fixture(2501);
        let directory = tempfile::tempdir().unwrap();
        let store =
            parquetviewer::settings::SettingsStore::at(directory.path().join("settings.json"));
        cx.update(|cx| {
            gpui_omarchy::init(cx);
            cx.bind_keys([gpui_kit::KeyBinding::new(
                "escape",
                super::CloseDetail,
                Some("ParquetViewer"),
            )]);
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
            assert!(!window.is_fullscreen());
            let viewport = window.viewport_size();
            window.click(("plot", 0usize), cx);
            window.render_frame(cx);
            assert!(!window.is_fullscreen());
            assert_eq!(window.viewport_size(), viewport);
            assert!(window.find("chart-detail").visible());
            assert!(window.try_find("horizontal-table").is_none());
            let detail = content.read(cx).detail.as_ref().unwrap();
            assert_eq!(detail.rows, 500);
            assert_eq!(detail.bars.len(), 32);
            window.click("close-chart", cx);
            window.render_frame(cx);
            assert!(!window.is_fullscreen());
            assert_eq!(window.viewport_size(), viewport);
            assert!(window.find("horizontal-table").visible());
            window.toggle_fullscreen();
            window.click(("plot", 0usize), cx);
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(content.read(cx).detail.is_none());
            assert!(
                window.is_fullscreen(),
                "An already fullscreen table must stay fullscreen"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn association_offer_is_opt_in_remembers_decline_and_can_be_reopened(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store =
            parquetviewer::settings::SettingsStore::at(directory.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(None, store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("associate").visible());
            assert!(!content.read(cx).association_busy);
            window.click("association-decline", cx);
            window.render_frame(cx);
            assert!(window.try_find("associate").is_none());
            assert!(store.load().unwrap().association_prompted);
            window.click("app-menu", cx);
            window.render_frame(cx);
            window.click(("menu-item", 2usize), cx);
            window.render_frame(cx);
            assert!(window.find("associate").visible());
            assert!(
                !content.read(cx).association_busy,
                "Showing the offer must not change defaults"
            );
            window.click("association-decline", cx);
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(None, store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("associate").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn sorts_pages_navigates_and_virtualizes_both_axes(cx: &mut TestAppContext) {
        let file = fixture(251);
        let page = load_page(file.path(), 0, 100).unwrap();
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(1280.0), px(800.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut view = Viewer::new(
                            None,
                            parquetviewer::settings::SettingsStore::at(
                                file.path().with_extension("settings.json"),
                            ),
                            window,
                            cx,
                        );
                        view.columns = (0..page.profiles.len()).collect();
                        view.rows = (0..page.frame.height()).collect();
                        view.settings.row_limit = parquetviewer::data::RowLimit::Paged(100);
                        view.page = Some(Arc::new(page));
                        view
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find(("column", 0usize)).visible());
            assert!(window.try_find(("column", 39usize)).is_none());
            assert!(window.find(("row", 0usize)).visible());
            assert!(window.try_find(("row", 99usize)).is_none());
            let first = window.find(("row", 0usize)).bounds();
            assert!(first.size.height >= px(30.0));
            assert!(first.top() >= window.find(("column", 0usize)).bounds().bottom());
            window.click(("sort", 0usize), cx);
            assert_eq!(content.read(cx).rows[0], 99);
            window.click(("sort", 0usize), cx);
            assert_eq!(content.read(cx).rows[0], 0);
            window.click(("sort", 0usize), cx);
            assert_eq!(content.read(cx).sort, None);
            assert_eq!(content.read(cx).columns[2], 2);
            let height = window.find(("column", 0usize)).bounds().size.height;
            toggle_plots(window, cx);
            window.render_frame(cx);
            assert!(!content.read(cx).settings.show_plots);
            assert!(window.find(("column", 0usize)).bounds().size.height < height);
            window.scroll(
                "horizontal-table",
                ScrollDelta::Pixels(point(px(-8000.0), px(0.0))),
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("column", 0usize)).is_none());
            assert!(window.find(("column", 36usize)).visible());
            window.click("next", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(!content.read(cx).loading);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 100);
            window.click(("page", 3usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let view = content.read(cx);
            assert_eq!(view.page.as_ref().unwrap().offset, 200);
            assert_eq!(view.rows.len(), 51);
            window.click("next", cx);
            assert!(
                !content.read(cx).loading,
                "Next must be disabled on the final page"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn wide_tables_scroll_rows_by_default_and_keep_axes_independent(cx: &mut TestAppContext) {
        let file = fixture(2501);
        let directory = tempfile::tempdir().unwrap();
        let store =
            parquetviewer::settings::SettingsStore::at(directory.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("column", 39usize)).is_none());
            window.scroll("horizontal-table", ScrollDelta::Lines(point(0.0, -3.0)), cx);
            let view = content.read(cx);
            assert_eq!(
                view.horizontal.offset().x,
                px(0.0),
                "Ordinary wheel input must not move columns"
            );
            assert!(
                view.vertical.0.borrow().base_handle.offset().y < px(0.0),
                "Ordinary wheel input must move rows"
            );
            // Even at the end of the rows, vertical input must not fall back to columns.
            for _ in 0..2 {
                window.scroll(
                    "horizontal-table",
                    ScrollDelta::Pixels(point(px(0.0), px(-100_000.0))),
                    cx,
                );
                assert_eq!(content.read(cx).horizontal.offset().x, px(0.0));
            }
            let y = content.read(cx).vertical.0.borrow().base_handle.offset().y;
            window.scroll("horizontal-table", ScrollDelta::Lines(point(-3.0, 0.0)), cx);
            assert!(
                content.read(cx).horizontal.offset().x < px(0.0),
                "Horizontal gestures must still move columns"
            );
            assert_eq!(
                content.read(cx).vertical.0.borrow().base_handle.offset().y,
                y,
                "Horizontal gestures must not move rows"
            );
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn resizing_refreshes_headers_and_idle_scrollbar_remains_clickable(cx: &mut TestAppContext) {
        let file = fixture(251);
        let path = file.path().to_owned();
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(1280.0), px(800.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        Viewer::new(
                            Some(path.clone()),
                            parquetviewer::settings::SettingsStore::at(
                                path.with_extension("settings.json"),
                            ),
                            window,
                            cx,
                        )
                    })
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("column", 14usize)).is_none());
        })
        .unwrap();
        cx.simulate_window_resize(handle, size(px(3840.0), px(2160.0)));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let header = window.find(("column", 14usize));
            let cell = window.find(("cell", 14usize));
            assert!(header.visible());
            assert_eq!(header.bounds().left(), cell.bounds().left());
        })
        .unwrap();
        cx.simulate_window_resize(handle, size(px(1000.0), px(600.0)));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(5));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(("column", 14usize)).is_none());
            let bounds = window.find("horizontal-table").bounds();
            // Click the scrollbar track after idle, with no preceding scroll/hover.
            window.click_at(
                "horizontal-table",
                point(bounds.size.width - px(30.0), bounds.size.height - px(5.0)),
                cx,
            );
            assert!(content.read(cx).horizontal.offset().x < px(-100.0));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let view = content.read(cx);
            let column = view.columns[view.visible_columns().start];
            assert!(window.find(("column", column)).visible());
            let cell = window.find(("cell", column));
            assert_eq!(
                window.find(("column", column)).bounds().left(),
                cell.bounds().left()
            );
        })
        .unwrap();
    }
    #[gpui_kit::test]
    fn saves_and_restores_controls_navigation_and_all_rows(cx: &mut TestAppContext) {
        use parquetviewer::{data::RowLimit, settings::SettingsStore};
        let file = fixture(2501);
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::at(directory.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let open = |cx: &mut TestAppContext| {
            cx.update(|cx| {
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| {
                        Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx)
                    })
                })
                .unwrap()
            })
        };
        let (handle, content) = open(cx);
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(("page-size", 0usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            toggle_plots(window, cx);
            window.click(("sort", 0usize), cx);
            window.click("next", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 100);
            assert_eq!(content.read(cx).rows[0], 99);
            assert!(!content.read(cx).settings.show_plots);
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        let settings = store.load().unwrap();
        assert_eq!(settings.row_limit, RowLimit::Paged(100));
        assert!(!settings.show_plots);
        assert_eq!(settings.files[0].offset, 100);
        assert_eq!(settings.files[0].sort.as_ref().unwrap().column, "column_0");

        let (handle, content) = open(cx);
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).page.as_ref().unwrap().offset, 100);
            assert_eq!(content.read(cx).rows[0], 99);
            assert!(window.find(("column", 0usize)).bounds().size.height <= px(104.0));
            window.click(("page-size", 3usize), cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(content.read(cx).rows.len(), 2501);
            assert_eq!(content.read(cx).rows[0], 2500);
            assert_eq!(content.read(cx).page.as_ref().unwrap().profile_rows, 1000);
            window.click(("sort", 0usize), cx);
            assert!(
                content.read(cx).loading,
                "All rows sorting must run off the UI thread"
            );
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert!(!content.read(cx).loading);
            assert_eq!(content.read(cx).rows[0], 0);
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        assert_eq!(store.load().unwrap().row_limit, RowLimit::All);
        let (handle, content) = open(cx);
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).rows.len(), 2501);
            assert_eq!(content.read(cx).rows[0], 0);
            assert!(!content.read(cx).settings.show_plots);
            window.remove_window();
        })
        .unwrap();
    }
    #[gpui_kit::test]
    fn document_open_during_loading_opens_the_requested_file(cx: &mut TestAppContext) {
        let first = fixture(251);
        let second = fixture(502);
        let directory = tempfile::tempdir().unwrap();
        let store =
            parquetviewer::settings::SettingsStore::at(directory.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let mut view = Viewer::new(Some(first.path().to_owned()), store, window, cx);
                    assert!(view.loading);
                    // Finder's open-document event can arrive while the initial read is pending.
                    view.open_path(second.path().to_owned(), window, cx);
                    assert!(view.pending_source.is_some());
                    view
                })
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let view = content.read(cx);
            assert!(!view.loading);
            assert!(view.pending_source.is_none());
            assert_eq!(
                view.page.as_ref().unwrap().path,
                std::fs::canonicalize(second.path()).unwrap()
            );
            assert_eq!(view.page.as_ref().unwrap().total_rows, 502);
            assert_eq!(view.rows.len(), 500);
            window.remove_window();
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn overview_search_inspection_details_and_clear_history(cx: &mut TestAppContext) {
        let file = fixture(2501);
        let dir = tempfile::tempdir().unwrap();
        let store = parquetviewer::settings::SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(file.path().to_owned()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            content.update(cx, |view, cx| {
                view.find_column(&super::FindColumn, window, cx);
                view.search_input
                    .update(cx, |input, cx| input.set_value("column_35", window, cx));
            });
            window.render_frame(cx);
            window.click(("find-column", 35usize), cx);
            window.render_frame(cx);
            assert!(content.read(cx).viewport_x < px(-6000.0));
            assert!(window.find(("column", 35usize)).visible());
            window.double_click(("cell", 35usize), cx);
            window.render_frame(cx);
            let super::Panel::Cell(detail) = content.read(cx).panel.as_ref().unwrap() else {
                panic!("cell inspector")
            };
            assert_eq!(detail.name, "column_35");
            assert_eq!(detail.source_row, 1);
            assert_eq!(detail.text, "2500");
            assert_eq!(content.read(cx).text_area.read(cx).value().as_ref(), "2500");
            window.click("copy-cell", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text(),
                Some("2500".to_string())
            );
            window.click("close-panel", cx);
            content.update(cx, |view, cx| view.show_info(window, cx));
            window.render_frame(cx);
            let text = content.read(cx).text_area.read(cx).value();
            assert!(text.contains("Schema (file order"));
            assert!(text.contains("row groups"));
            assert!(text.contains("Zstd"));
            assert!(text.contains("RLE"));
            window.click("close-panel", cx);
            content.update(cx, |view, cx| view.start_overview(window, cx));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(matches!(
                content.read(cx).panel,
                Some(super::Panel::Overview)
            ));
            assert_eq!(content.read(cx).overview.as_ref().unwrap().windows, 16);
            window.click(("overview-column", 0usize), cx);
            window.render_frame(cx);
            assert!(
                content
                    .read(cx)
                    .detail_scope
                    .as_ref()
                    .unwrap()
                    .contains("stratified preview")
            );
            window.click("close-chart", cx);
            window.render_frame(cx);
            assert!(matches!(
                content.read(cx).panel,
                Some(super::Panel::Overview)
            ));
            window.click("close-panel", cx);
            content.update(cx, |view, cx| {
                view.show_panel(super::Panel::Recent, window, cx)
            });
            window.render_frame(cx);
            assert!(window.find(("recent-file", 0usize)).visible());
            window.click("clear-history", cx);
            window.render_frame(cx);
            assert!(store.load().unwrap().files.is_empty());
            content.update(cx, |view, cx| {
                view.open_path(
                    "https://demo-user:synthetic-secret@example.com/file.parquet?token=synthetic-query".into(),
                    window,
                    cx,
                );
                let error = view.error.as_ref().unwrap();
                assert!(error.contains("credential profiles"));
                assert!(!error.contains("synthetic-secret"));
                assert!(!error.contains("synthetic-query"));
                assert!(!view.loading);
            });
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        assert!(store.load().unwrap().files.is_empty());
    }

    #[gpui_kit::test]
    fn csv_options_persist_and_reopen_the_file(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.csv");
        std::fs::write(&path, "id;label\n1;one\n2;two\n").unwrap();
        let store = parquetviewer::settings::SettingsStore::at(dir.path().join("settings.json"));
        cx.update(gpui_omarchy::init);
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(path.clone()), store.clone(), window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).page.as_ref().unwrap().frame.width(), 1);
            content.update(cx, |view, cx| view.show_csv_options(window, cx));
            window.render_frame(cx);
            window.click(("delimiter", 3usize), cx);
            window.click("csv-types", cx);
            window.click("apply-csv", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let page = content.read(cx).page.as_ref().unwrap();
            assert_eq!(page.frame.width(), 2);
            assert_eq!(page.profiles[0].dtype, "str");
            assert_eq!(page.cell(0, 1).0, "one");
            window.remove_window();
        })
        .unwrap();
        drop(content);
        cx.run_until_parked();
        assert_eq!(
            store.load().unwrap().csv_options.delimiter,
            parquetviewer::source::Delimiter::Semicolon
        );
        let (handle, content) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| Viewer::new(Some(path), store, window, cx))
            })
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(content.read(cx).page.as_ref().unwrap().frame.width(), 2);
            window.remove_window();
        })
        .unwrap();
    }
}
