#!/usr/bin/env python3
"""Collect maintained cloud downloads and create a checksummed local release draft."""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent


def digest(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def prepare(windows_build_commit=None, exclude_untracked=()):
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    names = [f"tablelamp-{version}-linux-x64-cloud.tar.gz",
             f"tablelamp-{version}-windows-x64-cloud.zip",
             f"tablelamp-{version}-windows-x64-cloud-setup.exe",
             f"tablelamp-bin-{version.replace('-', '.')}-1-x86_64.pkg.tar.zst"]
    for name in names:
        path = ROOT / "dist" / name
        if not path.is_file():
            raise SystemExit(f"Missing native package: {name}")
        expected = (path.parent / (name + ".sha256")).read_text(encoding="utf-8-sig").split()
        if expected != [digest(path), name]:
            raise SystemExit(f"Package checksum mismatch: {name}")

    destination = ROOT / "dist" / f"release-{version}"
    destination.mkdir(parents=True, exist_ok=True)
    checksums = []
    for name in names:
        shutil.copy2(ROOT / "dist" / name, destination / name)
        shutil.copy2(ROOT / "dist" / (name + ".sha256"), destination / (name + ".sha256"))
        checksums.append(f"{digest(destination / name)}  {name}\n")
    (destination / "SHA256SUMS").write_text("".join(checksums), encoding="utf-8")
    shutil.copy2(ROOT / "docs" / "release-notes.md", destination / "RELEASE_NOTES.md")
    # Keep provenance honest while a release is still being reviewed locally.
    base = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    tracked = subprocess.check_output(["git", "ls-files", "-z", "--cached"], cwd=ROOT).decode().split("\0")
    untracked = subprocess.check_output(["git", "ls-files", "-z", "--others", "--exclude-standard"], cwd=ROOT).decode().split("\0")
    excluded = set(exclude_untracked)
    if excluded - set(untracked):
        raise SystemExit("Only existing untracked files can be excluded from release provenance")
    paths = tracked + [path for path in untracked if path not in excluded]
    source_hashes = [f"{digest(ROOT / path)}  {path}\n" for path in sorted(set(paths)) if path and (ROOT / path).is_file()]
    fingerprint = hashlib.sha256("".join(source_hashes).encode()).hexdigest()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=ROOT))
    dirty = dirty or any(path for path in untracked if path not in excluded)
    manifest = {"version": version, "base_commit": base, "uncommitted_source": dirty,
                "source_fingerprint": fingerprint,
                "release_commit": None if dirty else base, "assets": names}
    if excluded:
        manifest["excluded_untracked_files"] = sorted(excluded)
    if windows_build_commit:
        windows_build_commit = subprocess.check_output(
            ["git", "rev-parse", "--verify", windows_build_commit + "^{commit}"],
            cwd=ROOT, text=True).strip()
        manifest["windows_build_commit"] = windows_build_commit
    (destination / "SOURCE.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Prepared local draft: {destination}")
    if dirty:
        print("Source is uncommitted. Commit and repeat native checks as needed before tagging or publishing.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--windows-build-commit", help="Record the source commit of a reused native Windows build")
    parser.add_argument("--exclude-untracked", action="append", default=[],
                        help="Exclude an unrelated untracked file that is not packaged (repeatable)")
    args = parser.parse_args()
    prepare(args.windows_build_commit, args.exclude_untracked)
