#!/usr/bin/env python3
"""Collect the three maintained cloud downloads and create a local release draft."""
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


def prepare():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    names = [f"rowlight-{version}-linux-x64-cloud.tar.gz",
             f"rowlight-{version}-windows-x64-cloud.zip",
             f"rowlight-{version}-windows-x64-cloud-setup.exe"]
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
        checksums.append(f"{digest(destination / name)}  {name}\n")
    (destination / "SHA256SUMS").write_text("".join(checksums), encoding="utf-8")
    shutil.copy2(ROOT / "docs" / "release-notes.md", destination / "RELEASE_NOTES.md")
    # Keep provenance honest while a release is still being reviewed locally.
    base = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    paths = subprocess.check_output(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT).decode().split("\0")
    source_hashes = [f"{digest(ROOT / path)}  {path}\n" for path in sorted(set(paths)) if path and (ROOT / path).is_file()]
    fingerprint = hashlib.sha256("".join(source_hashes).encode()).hexdigest()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))
    manifest = {"version": version, "base_commit": base, "uncommitted_source": dirty,
                "source_fingerprint": fingerprint,
                "release_commit": None if dirty else base, "assets": names}
    (destination / "SOURCE.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Prepared local draft: {destination}")
    if dirty:
        print("Source is uncommitted. Commit and repeat native checks as needed before tagging or publishing.")


if __name__ == "__main__":
    prepare()
