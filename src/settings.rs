//! Small, atomic per-user preferences; no database or background service.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    data::RowLimit,
    source::{CsvOptions, Location},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Sort {
    pub column: String,
    pub descending: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct FileView {
    pub path: PathBuf,
    pub location: Option<Location>,
    pub csv_options: Option<CsvOptions>,
    pub offset: usize,
    pub sort: Option<Sort>,
    pub horizontal: f32,
    pub row: usize,
    pub widths: BTreeMap<String, f32>,
    pub hidden_columns: Vec<String>,
    pub filters: Vec<crate::filter::Filter>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub row_limit: RowLimit,
    pub show_plots: bool,
    pub association_prompted: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub files: Vec<FileView>,
    pub csv_options: CsvOptions,
    pub aws_profile: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            row_limit: RowLimit::default(),
            show_plots: true,
            association_prompted: false,
            window_width: 1280.0,
            window_height: 800.0,
            files: Vec::new(),
            csv_options: CsvOptions::default(),
            aws_profile: String::new(),
        }
    }
}

impl Settings {
    pub fn file(&self, path: &Path) -> Option<&FileView> {
        self.files.iter().find(|file| file.path == path)
    }

    pub fn file_location(&self, location: &Location) -> Option<&FileView> {
        self.files.iter().find(|file| {
            file.location.as_ref().map_or_else(
                || matches!(location, Location::Local(path) if path == &file.path),
                |saved| saved == location,
            )
        })
    }

    pub fn remember(&mut self, file: FileView) {
        // JSON paths must be Unicode; browsing non-UTF-8 paths still works.
        if file.path.to_str().is_none() || file.location.as_ref().is_some_and(|l| !l.rememberable())
        {
            return;
        }
        self.files.retain(|entry| entry.path != file.path);
        self.files.insert(0, file);
        self.files.truncate(20);
    }

    fn sanitize(&mut self) {
        if !self.row_limit.valid() {
            self.row_limit = RowLimit::default();
        }
        if !self.window_width.is_finite() || !self.window_height.is_finite() {
            self.window_width = 1280.0;
            self.window_height = 800.0;
        }
        self.window_width = self.window_width.clamp(1000.0, 7680.0);
        self.window_height = self.window_height.clamp(500.0, 4320.0);
        self.files
            .retain(|f| f.location.as_ref().is_none_or(Location::rememberable));
        self.files.truncate(20);
        self.aws_profile
            .truncate(self.aws_profile.floor_char_boundary(128));
        for file in &mut self.files {
            file.horizontal = file.horizontal.min(0.0);
            file.widths.retain(|_, width| width.is_finite());
            for width in file.widths.values_mut() {
                *width = width.clamp(120.0, 800.0);
            }
            file.filters.truncate(8);
        }
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn user() -> Result<Self> {
        if let Some(path) =
            std::env::var_os("PARQUETVIEWER_SETTINGS_PATH").filter(|path| !path.is_empty())
        {
            return Ok(Self::at(PathBuf::from(path)));
        }
        let root = dirs::config_dir().context("Cannot locate your user preferences directory")?;
        Ok(Self::at(root.join("parquetviewer/settings.json")))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Settings> {
        let contents = match fs::read(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::default());
            }
            Err(error) => return Err(error).context("Cannot read saved preferences"),
        };
        let mut settings: Settings = serde_json::from_slice(&contents)
            .with_context(|| format!("Cannot read {}", self.path.display()))?;
        settings.sanitize();
        Ok(settings)
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("Preferences path has no parent")?;
        fs::create_dir_all(parent).context("Cannot create preferences directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, settings)?;
        temporary.write_all(b"\n")?;
        temporary.flush()?;
        temporary
            .persist(&self.path)
            .context("Cannot save preferences")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_sources_never_enter_saved_preferences() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = SettingsStore::at(dir.path().join("settings.json"));
        let mut settings = Settings::default();
        settings.remember(FileView {
            path: PathBuf::from("https://example.com/file.parquet"),
            location: Some(Location::remote(
                "https://example.com/file.parquet?token=private-secret",
            )?),
            ..Default::default()
        });
        store.save(&settings)?;
        assert!(settings.files.is_empty());
        assert!(
            !std::fs::read_to_string(dir.path().join("settings.json"))?.contains("private-secret")
        );
        Ok(())
    }

    #[test]
    fn preferences_round_trip_and_keep_recent_file_views() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::at(directory.path().join("app/settings.json"));
        assert_eq!(store.load()?, Settings::default());
        let mut settings = Settings {
            row_limit: RowLimit::All,
            show_plots: false,
            ..Settings::default()
        };
        for i in 0..25 {
            settings.remember(FileView {
                path: PathBuf::from(format!("/file-{i}.parquet")),
                offset: 500,
                sort: Some(Sort {
                    column: "amount".into(),
                    descending: true,
                }),
                ..FileView::default()
            });
        }
        store.save(&settings)?;
        assert_eq!(store.load()?, settings);
        settings.row_limit = RowLimit::Paged(100);
        store.save(&settings)?;
        assert_eq!(store.load()?, settings);
        assert_eq!(settings.files.len(), 20);
        assert!(settings.file(Path::new("/file-24.parquet")).is_some());
        assert!(settings.file(Path::new("/file-0.parquet")).is_none());
        Ok(())
    }

    #[test]
    fn old_invalid_and_corrupt_preferences_are_handled() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.json");
        let store = SettingsStore::at(path.clone());
        fs::write(
            &path,
            br#"{"row_limit":{"mode":"paged","rows":0},"window_width":4,"unknown":true}"#,
        )?;
        let settings = store.load()?;
        assert_eq!(settings.row_limit, RowLimit::default());
        assert_eq!(settings.window_width, 1000.0);
        assert!(settings.show_plots);
        fs::write(path, "not json")?;
        assert!(store.load().is_err());
        Ok(())
    }
}
