//! Development-only runner. Its dependencies are not linked into the viewer.
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, ExitStatus},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Snapshot = BTreeMap<PathBuf, (SystemTime, u64)>;
const POLL: Duration = Duration::from_millis(100);
const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Default)]
struct Options {
    core: bool,
    once: bool,
    help: bool,
    arguments: Vec<OsString>,
}

fn options(arguments: impl IntoIterator<Item = OsString>) -> Result<Options> {
    let mut options = Options::default();
    let mut positional = false;
    for argument in arguments {
        if positional {
            options.arguments.push(argument);
        } else {
            match argument.to_str() {
                Some("--") => positional = true,
                Some("--core") => options.core = true,
                Some("--once") => options.once = true,
                Some("--help" | "-h") => options.help = true,
                Some(value) if value.starts_with('-') => {
                    return Err(format!("Unknown development option: {value}").into());
                }
                _ => options.arguments.push(argument),
            }
        }
    }
    if options.arguments.len() > 1 {
        return Err("Pass one file or URL to the preview".into());
    }
    Ok(options)
}

fn collect(path: &Path, snapshot: &mut Snapshot) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), snapshot)?;
        }
    } else if metadata.is_file() {
        snapshot.insert(path.to_owned(), (metadata.modified()?, metadata.len()));
    }
    Ok(())
}

fn snapshot(root: &Path) -> io::Result<Snapshot> {
    let mut result = Snapshot::new();
    for path in [
        "src",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        ".cargo/config.toml",
        "rust-toolchain.toml",
        "packaging/icon.png",
        "packaging/windows/app.manifest",
        "packaging/windows/icon.ico",
    ] {
        collect(&root.join(path), &mut result)?;
    }
    Ok(result)
}

struct Process {
    child: Child,
    directory: Option<PathBuf>,
}

impl Process {
    fn spawn(mut command: Command, directory: Option<PathBuf>) -> io::Result<Self> {
        // Keep only this runner's children in each group, including rustc workers.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                if let Some(directory) = &directory {
                    let _ = fs::remove_dir_all(directory);
                }
                return Err(error);
            }
        };
        Ok(Self { child, directory })
    }

    fn stop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        #[cfg(unix)]
        {
            use nix::{
                sys::signal::{Signal, killpg},
                unistd::Pid,
            };
            let _ = killpg(Pid::from_raw(self.child.id() as i32), Signal::SIGTERM);
        }
        #[cfg(windows)]
        {
            use std::process::Stdio;
            let _ = Command::new("taskkill")
                .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        #[cfg(unix)]
        {
            use nix::{
                sys::signal::{Signal, killpg},
                unistd::Pid,
            };
            let _ = killpg(Pid::from_raw(self.child.id() as i32), Signal::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
        if let Some(directory) = &self.directory {
            let _ = fs::remove_dir_all(directory);
        }
    }
}

fn target_directory(root: &Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => root.join(path),
        None => root.join("target"),
    }
}

fn build(root: &Path, target: &Path, options: &Options) -> io::Result<Process> {
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    // cargo run injects xtask's package metadata. Dependencies such as ring
    // track those variables, so leaking them makes ordinary Cargo builds dirty.
    // Keep user configuration (CARGO_TARGET_DIR, CARGO_PROFILE_*, RUSTFLAGS, …).
    for (key, _) in env::vars_os() {
        if let Some(name) = key.to_str()
            && (name.starts_with("CARGO_PKG_")
                || name.starts_with("CARGO_CFG_")
                || name.starts_with("CARGO_FEATURE_")
                || matches!(
                    name,
                    "CARGO_MANIFEST_DIR"
                        | "CARGO_MANIFEST_PATH"
                        | "CARGO_MANIFEST_LINKS"
                        | "CARGO_BIN_NAME"
                        | "CARGO_CRATE_NAME"
                ))
        {
            command.env_remove(key);
        }
    }
    command
        .current_dir(root)
        .args([
            "build",
            "--locked",
            "--package",
            "parquetviewer",
            "--bin",
            "parquetviewer",
            "--profile",
            "dev",
            "--target-dir",
        ])
        .arg(target);
    if !options.core {
        command.args(["--features", "cloud"]);
    }
    Process::spawn(command, None)
}

fn preview(target: &Path, state: &Path, options: &Options, generation: u64) -> io::Result<Process> {
    let name = format!("parquetviewer{}", env::consts::EXE_SUFFIX);
    let directory = state.join(format!("preview-{}-{generation}", std::process::id()));
    fs::create_dir_all(&directory)?;
    let executable = directory.join(&name);
    // A separate copy also lets Cargo replace the build artifact on Windows.
    if let Err(error) = fs::copy(target.join("debug").join(name), &executable) {
        let _ = fs::remove_dir_all(directory);
        return Err(error);
    }
    let mut command = Command::new(executable);
    command
        .args(&options.arguments)
        .env("PARQUETVIEWER_SETTINGS_PATH", state.join("settings.json"));
    Process::spawn(command, Some(directory))
}

fn code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

fn dev(options: Options) -> Result<i32> {
    if options.help {
        println!(
            "Usage: cargo dev [--core] [--once] [--] [FILE|URL]\n\nBuild an incremental development preview, then restart after successful source edits.\nCloud features are enabled by default. --once builds/runs without watching.\nClose the preview or press Ctrl+C to stop. Preferences are isolated under target/dev-preview."
        );
        return Ok(0);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let target = target_directory(root);
    let state = target
        .join("dev-preview")
        .join(if options.core { "core" } else { "cloud" });
    fs::create_dir_all(&state)?;
    let preferences = state.join("settings.json");
    if !preferences.exists() {
        fs::write(preferences, "{\"association_prompted\":true}\n")?;
    }
    let stopping = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopping);
    ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))?;
    let mut seen = snapshot(root)?;
    let mut last_edit = Instant::now() - DEBOUNCE;
    let mut pending = true;
    let mut building: Option<(Process, Instant)> = None;
    let mut app: Option<Process> = None;
    let mut generation = 0;
    if !options.once {
        println!("Watching app sources. Close the preview or press Ctrl+C to stop.");
    }
    loop {
        if stopping.load(Ordering::Relaxed) {
            return Ok(130);
        }
        if !options.once {
            let current = snapshot(root)?;
            if current != seen {
                seen = current;
                pending = true;
                last_edit = Instant::now();
            }
        }
        if let Some(process) = &mut app
            && let Some(status) = process.child.try_wait()?
        {
            return Ok(code(status));
        }
        if let Some((process, started)) = &mut building
            && let Some(status) = process.child.try_wait()?
        {
            let elapsed = started.elapsed();
            building = None;
            if status.success() && !pending {
                drop(app.take());
                generation += 1;
                app = Some(preview(&target, &state, &options, generation)?);
                println!("Preview ready (build {:.2}s).", elapsed.as_secs_f64());
            } else if !status.success() {
                if options.once {
                    return Ok(code(status));
                }
                eprintln!(
                    "Build failed. Waiting for a source edit; any existing preview stays open."
                );
            }
        }
        if pending && building.is_none() && last_edit.elapsed() >= DEBOUNCE {
            pending = false;
            let started = Instant::now();
            building = Some((build(root, &target, &options)?, started));
        }
        thread::sleep(POLL);
    }
}

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let result = if arguments.next().as_deref() == Some(std::ffi::OsStr::new("dev")) {
        options(arguments).and_then(dev)
    } else {
        Err("Usage: cargo dev [--core] [--once] [--] [FILE|URL]".into())
    };
    match result {
        Ok(value) => ExitCode::from(u8::try_from(value).unwrap_or(1)),
        Err(error) => {
            eprintln!("dev: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_paths_keep_spaces_unicode_and_app_flags() {
        let args = ["--core", "--once", "--", "résultats 100%.parquet"].map(OsString::from);
        let parsed = options(args).unwrap();
        assert!(parsed.core && parsed.once);
        assert_eq!(parsed.arguments, [OsString::from("résultats 100%.parquet")]);
        assert!(options(["--".into(), "--help".into()]).unwrap().arguments[0] == "--help");
    }

    #[test]
    fn snapshot_detects_atomic_replacements_new_and_deleted_sources() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "parquetviewer-watch-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.rs"), "one").unwrap();
        let first = snapshot(&root).unwrap();
        fs::write(root.join("replacement"), "second version").unwrap();
        fs::rename(root.join("replacement"), root.join("src/a.rs")).unwrap();
        assert_ne!(snapshot(&root).unwrap(), first);
        fs::write(root.join("src/new.rs"), "new").unwrap();
        assert_eq!(snapshot(&root).unwrap().len(), 2);
        fs::remove_file(root.join("src/a.rs")).unwrap();
        assert_eq!(snapshot(&root).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
