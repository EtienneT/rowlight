//! File defaults change only after the user chooses the first-run/menu action.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use anyhow::ensure;
use anyhow::{Context, Result};
use std::process::Command;

pub fn associate() -> Result<String> {
    #[cfg(target_os = "linux")]
    {
        let desktop = dirs::data_dir()
            .context("Cannot find the applications directory")?
            .join("applications/com.parquetviewer.ParquetViewer.desktop");
        ensure!(
            desktop.is_file(),
            "Install Parquet Viewer first, then choose this action again."
        );
        std::fs::create_dir_all(
            dirs::config_dir().context("Cannot find the configuration directory")?,
        )?;
        let output = Command::new("xdg-mime")
            .args([
                "default",
                "com.parquetviewer.ParquetViewer.desktop",
                "application/vnd.apache.parquet",
                "application/x-parquet",
            ])
            .output()
            .context("Cannot start xdg-mime")?;
        ensure!(
            output.status.success(),
            "Cannot set the file default: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual = Command::new("xdg-mime")
            .args(["query", "default", "application/vnd.apache.parquet"])
            .output()?;
        ensure!(
            actual.status.success()
                && String::from_utf8_lossy(&actual.stdout).trim()
                    == "com.parquetviewer.ParquetViewer.desktop",
            "The desktop did not accept the file default. Choose Parquet Viewer in your file manager's Open With dialog."
        );
        Ok("Parquet files now open in Parquet Viewer.".into())
    }
    #[cfg(target_os = "windows")]
    {
        // Windows protects UserChoice. The OS presents the final selection.
        Command::new("explorer.exe")
            .arg("ms-settings:defaultapps?registeredAppUser=Parquet%20Viewer")
            .spawn()
            .context("Cannot open Windows Default Apps")?;
        Ok("In Windows Default Apps, select Parquet Viewer for .parquet and .parq. On Windows 10, search for .parquet in Default Apps.".into())
    }
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe()?;
        let helper = executable
            .parent()
            .context("Cannot locate the app bundle")?
            .join("parquetviewer-associate");
        ensure!(
            helper.is_file(),
            "Install the macOS application bundle first, then choose this action again."
        );
        let output = Command::new(helper)
            .output()
            .context("Cannot open the macOS file association helper")?;
        ensure!(
            output.status.success(),
            "macOS could not change the file default: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok("Parquet files now open in Parquet Viewer.".into())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    anyhow::bail!("Choose Parquet Viewer in your file manager's Open With dialog.");
}
