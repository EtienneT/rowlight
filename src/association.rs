//! File defaults change only after the user chooses the first-run/menu action.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use anyhow::ensure;
use anyhow::{Context, Result};
use std::process::Command;

pub fn associate() -> Result<String> {
    #[cfg(target_os = "linux")]
    {
        let data_home = dirs::data_dir().context("Cannot find the applications directory")?;
        let data_dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        ensure!(
            desktop_registered(&data_home, std::env::split_paths(&data_dirs)),
            "Install Tablelamp first, then choose this action again."
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
            "The desktop did not accept the file default. Choose Tablelamp in your file manager's Open With dialog."
        );
        Ok("Parquet files now open in Tablelamp.".into())
    }
    #[cfg(target_os = "windows")]
    {
        // Windows protects UserChoice. The OS presents the final selection.
        Command::new("explorer.exe")
            .arg("ms-settings:defaultapps?registeredAppUser=Parquet%20Viewer")
            .spawn()
            .context("Cannot open Windows Default Apps")?;
        Ok("In Windows Default Apps, select Tablelamp for .parquet and .parq. On Windows 10, search for .parquet in Default Apps.".into())
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
        Ok("Parquet files now open in Tablelamp.".into())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    anyhow::bail!("Choose Tablelamp in your file manager's Open With dialog.");
}

#[cfg(target_os = "linux")]
fn desktop_registered(
    data_home: &std::path::Path,
    data_dirs: impl Iterator<Item = std::path::PathBuf>,
) -> bool {
    std::iter::once(data_home.to_path_buf())
        .chain(data_dirs.filter(|path| path.is_absolute()))
        .any(|path| {
            path.join("applications/com.parquetviewer.ParquetViewer.desktop")
                .is_file()
        })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::desktop_registered;

    #[test]
    fn associations_recognize_user_and_system_package_installations() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        let system = root.path().join("system");
        assert!(!desktop_registered(&user, std::iter::once(system.clone())));

        let applications = system.join("applications");
        std::fs::create_dir_all(&applications).unwrap();
        let desktop = applications.join("com.parquetviewer.ParquetViewer.desktop");
        std::fs::write(&desktop, "[Desktop Entry]\nName=Tablelamp\n").unwrap();
        assert!(desktop_registered(&user, std::iter::once(system)));

        std::fs::create_dir_all(user.join("applications")).unwrap();
        std::fs::rename(
            desktop,
            user.join("applications/com.parquetviewer.ParquetViewer.desktop"),
        )
        .unwrap();
        assert!(desktop_registered(&user, std::iter::empty()));
    }
}
