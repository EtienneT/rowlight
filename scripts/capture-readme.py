#!/usr/bin/env python3
"""Capture the real Linux app with synthetic data and isolated themes/preferences."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


ROOT = Path(__file__).resolve().parents[1]
THEMES = {
    "tokyo-night": "Tokyo Night",
    "catppuccin": "Catppuccin",
    "gruvbox": "Gruvbox",
    "flexoki-light": "Flexoki Light",
}
WIDTHS = {"id": 160, "amount": 220, "active": 160, "region": 220, "note": 250, "created_at": 200}


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--xvfb", default=shutil.which("Xvfb"))
    parser.add_argument("--xdotool", default=shutil.which("xdotool"))
    parser.add_argument("--themes-dir", type=Path, default=Path("/usr/share/omarchy/themes"))
    args = parser.parse_args()
    if not args.xvfb or not args.xdotool or not all(shutil.which(t) for t in ("magick", "ffmpeg")):
        parser.error("Requires Xvfb, xdotool, ImageMagick, and FFmpeg; explicit tool paths are supported.")
    for slug in THEMES:
        if not (args.themes_dir / slug / "colors.toml").is_file():
            parser.error(f"Missing Omarchy palette: {slug}; supply --themes-dir.")

    work = ROOT / "target/readme-preview"
    work.mkdir(parents=True, exist_ok=True)
    assets = ROOT / "docs/assets"
    assets.mkdir(parents=True, exist_ok=True)
    # Stable cloud/dev features reuse the ordinary incremental build cache.
    subprocess.run(["cargo", "dev-build"], cwd=ROOT, check=True)
    source = work / "tablelamp-demo.parquet"
    if not source.exists():
        subprocess.run(["cargo", "run", "--locked", "--example", "sample", "--features", "cloud", "--", str(source), "50000"], cwd=ROOT, check=True)

    display = next((f":{n}" for n in range(190, 220) if not Path(f"/tmp/.X{n}-lock").exists() and not Path(f"/tmp/.X11-unix/X{n}").exists()), None)
    if display is None:
        parser.error("No free private X11 display in the capture range.")
    env = dict(os.environ)
    private_lib = Path(args.xdotool).resolve().parent.parent / "lib"
    if (private_lib / "libxdo.so.4").exists():
        env["LD_LIBRARY_PATH"] = str(private_lib) + (":" + env["LD_LIBRARY_PATH"] if env.get("LD_LIBRARY_PATH") else "")
    env.update(DISPLAY=display, WAYLAND_DISPLAY="", XDG_SESSION_TYPE="x11", LIBGL_ALWAYS_SOFTWARE="1")
    env["PARQUETVIEWER_SETTINGS_PATH"] = str(work / "settings.json")
    server = app = recorder = None

    def run(*command, **kwargs):
        return subprocess.run(command, env=env, check=True, **kwargs)

    def click(x, y):
        run(args.xdotool, "mousemove", str(x), str(y), "click", "1")
        time.sleep(0.35)

    def key(binding):
        run(args.xdotool, "key", "--clearmodifiers", binding)
        time.sleep(0.35)

    def idle(seconds=2):
        run(args.xdotool, "mousemove", "960", "830")
        time.sleep(seconds)

    def capture(name):
        run("magick", "import", "-display", display, "-window", window, str(assets / name))

    try:
        with (work / "xvfb.log").open("w") as log:
            server = subprocess.Popen([args.xvfb, display, "-screen", "0", "1400x860x24", "-nolisten", "tcp"], env=env, stdout=log, stderr=subprocess.STDOUT)
        time.sleep(0.5)
        if server.poll() is not None:
            raise RuntimeError(f"Xvfb failed; see {work / 'xvfb.log'}")
        for slug, title in THEMES.items():
            stop(app)
            current = work / "themes" / slug
            (current / "theme").mkdir(parents=True, exist_ok=True)
            shutil.copyfile(args.themes_dir / slug / "colors.toml", current / "theme/colors.toml")
            (current / "theme.name").write_text(title + "\n")
            env["TABLELAMP_PREVIEW_THEME"] = str(current)
            (work / "settings.json").write_text(json.dumps({
                "association_prompted": True, "row_limit": {"mode": "paged", "rows": 500},
                "show_plots": True, "window_width": 1400, "window_height": 860,
                "files": [{"path": str(source), "widths": WIDTHS}],
            }))
            with (work / f"{slug}.log").open("w") as log:
                app = subprocess.Popen([str(ROOT / "target/debug/parquetviewer"), str(source)], env=env, stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 10
            window = ""
            while time.monotonic() < deadline:
                if app.poll() is not None:
                    raise RuntimeError(f"App exited; see {work / (slug + '.log')}")
                found = subprocess.run([args.xdotool, "search", "--onlyvisible", "--pid", str(app.pid)], env=env, capture_output=True, text=True)
                if found.returncode == 0 and found.stdout.strip():
                    window = found.stdout.splitlines()[0]
                    break
                time.sleep(0.1)
            if not window:
                raise RuntimeError("Preview window did not appear within ten seconds.")
            run(args.xdotool, "windowfocus", window)
            idle(1)
            capture(f"{slug}.png")
            print(f"Captured {title}", flush=True)

            if slug == "tokyo-night":
                geometry = run(args.xdotool, "getwindowgeometry", "--shell", window, capture_output=True, text=True).stdout
                values = dict(line.split("=", 1) for line in geometry.splitlines())
                video_size = f"{values['WIDTH']}x{values['HEIGHT']}"
                with (work / "recording.log").open("w") as log:
                    recorder = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab", "-framerate", "10", "-video_size", video_size, "-window_id", window, "-i", display, "-an", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", str(work / "demo.mp4")], env=env, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT)
                idle()
                click(355, 190)  # Numeric distribution, expanded inside the app.
                run(args.xdotool, "mousemove", "355", "350")
                time.sleep(2.5)
                capture("column-detail.png")
                key("Escape")
                click(970, 322)  # Complete scalar value in the cell inspector.
                key("Return")
                idle(2)
                capture("cell-inspector.png")
                key("Escape")
                key("ctrl+Next")  # Native keyboard shortcut: next file page.
                idle(2)
                key("ctrl+f")
                run(args.xdotool, "type", "--clearmodifiers", "region")
                idle(2)
                capture("find-column.png")
                key("Escape")
                click(40, 35)
                time.sleep(0.5)
                click(110, 108)  # Collapse the plot headers through the real menu.
                idle(2)
                capture("compact-table.png")
                click(40, 35)
                click(110, 108)
                key("ctrl+Home")
                key("Escape")
                idle(1.5)
                recorder.stdin.write(b"q\n")
                recorder.stdin.flush()
                recorder.wait(timeout=10)
                if recorder.returncode:
                    raise RuntimeError(f"Recording failed; see {work / 'recording.log'}")
                recorder = None
        run("ffmpeg", "-y", "-loglevel", "error", "-i", str(work / "demo.mp4"), "-filter_complex", "fps=8,scale=1000:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle", "-loop", "0", str(assets / "tablelamp-demo.gif"))
        print(f"Media written to {assets}")
    finally:
        stop(recorder)
        stop(app)
        stop(server)


if __name__ == "__main__":
    main()
