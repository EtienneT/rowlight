#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod viewer;

use std::{ffi::OsStr, path::PathBuf};

use gpui_kit::*;
use parquetviewer::settings::SettingsStore;
use viewer::{CloseDetail, OpenFile, Quit, Viewer};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = match args.next() {
        Some(arg) if arg == OsStr::new("--help") || arg == OsStr::new("-h") => {
            println!(
                "Parquet Viewer\n\nUsage: parquetviewer [FILE|URL]\n\nOpen a file, or start without a path to use the file picker.\nCtrl/Cmd+O: open file · Ctrl/Cmd+F: find column · Home/End: first/last loaded row · Page Up/Down: screenful · Ctrl/Cmd+Page Up/Down or Alt+Left/Right: previous/next page · Ctrl/Cmd+Home/End: file start/end · Ctrl/Cmd+G: go to page · F1: keyboard shortcuts · Esc: clear selection/back · Ctrl/Cmd+Q: quit"
            );
            return Ok(());
        }
        Some(arg) => Some(PathBuf::from(arg)),
        None => None,
    };
    anyhow::ensure!(
        args.next().is_none(),
        "Open one file at a time. Usage: parquetviewer [FILE|URL]"
    );
    let store = SettingsStore::user()?;
    let settings = store.load().unwrap_or_default();
    let application = gpui_kit::application().with_assets(assets::Assets);
    let (open_sender, open_receiver) = async_channel::unbounded();
    application.on_open_urls(move |urls| {
        // A Finder document-open event may arrive before or after launch.
        if let Some(path) = urls
            .iter()
            .find_map(|url| parquetviewer::launch::file_from_url(url))
        {
            let _ = open_sender.try_send(path);
        }
    });
    application.on_reopen(|cx| cx.activate(true));
    application.run(move |cx| {
        gpui_omarchy::init(cx);
        #[cfg(target_os = "macos")]
        cx.set_menus(vec![
            Menu::new("Parquet Viewer").items([MenuItem::action("Quit Parquet Viewer", Quit)]),
            Menu::new("File").items([MenuItem::action("Open file…", OpenFile)]),
        ]);
        cx.bind_keys([
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-o"
                } else {
                    "ctrl-o"
                },
                OpenFile,
                Some("ParquetViewer"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-f"
                } else {
                    "ctrl-f"
                },
                viewer::FindColumn,
                Some("ParquetViewer"),
            ),
            KeyBinding::new("escape", CloseDetail, Some("ParquetViewer")),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                Some("ParquetViewer"),
            ),
        ]);
        viewer::bind_table_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(settings.window_width), px(settings.window_height)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Parquet Viewer".into()),
                ..Default::default()
            }),
            app_id: Some("com.parquetviewer.ParquetViewer".into()),
            window_min_size: Some(size(px(1000.0), px(500.0))),
            ..Default::default()
        };
        match gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Viewer::new(path, store, window, cx))
        }) {
            Ok((handle, content)) => {
                let content = content.downgrade();
                cx.spawn(async move |cx| {
                    while let Ok(path) = open_receiver.recv().await {
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = content.update(cx, |this, cx| this.open_path(path, window, cx));
                            window.activate_window();
                            cx.activate(true);
                        });
                    }
                })
                .detach();
            }
            Err(error) => {
                eprintln!("Cannot open Parquet Viewer: {error:#}");
                cx.quit();
            }
        }
        cx.activate(true);
    });
    Ok(())
}
