#!/usr/bin/env python3
"""Generate distributable notices, without exporting Cargo's local source paths.

Requires cargo-about 0.9.2 (a maintainer tool, not an application dependency).
"""
import argparse
import hashlib
import html
import json
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def escape(value):
    return html.escape(str(value), quote=True)


def generate(tool):
    version = subprocess.check_output([tool, "--version"], text=True).strip()
    if version != "cargo-about 0.9.2":
        raise SystemExit(f"Expected cargo-about 0.9.2; got {version}")
    with tempfile.TemporaryDirectory(prefix="tablelamp-notices-") as temporary:
        output = pathlib.Path(temporary) / "licenses.json"
        subprocess.run(
            [tool, "generate", "--locked", "--features", "cloud", "--fail",
             "--format", "json", "--output-file", str(output)],
            cwd=ROOT, check=True,
        )
        data = json.loads(output.read_text(encoding="utf-8"))

    lock_hash = hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()
    sections = ["""<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Tablelamp third-party notices</title>
<style>body{font:16px system-ui;max-width:960px;margin:2rem auto;padding:0 1rem}
pre{white-space:pre-wrap;overflow-wrap:anywhere}summary{cursor:pointer}
@media(prefers-color-scheme:dark){body{background:#202328;color:#eee}a{color:#8ac}}</style>
<h1>Tablelamp third-party notices</h1>
<p>Generated with cargo-about 0.9.2 for the Linux x64 and Windows x64 cloud builds.
Core downloads use a subset of these dependencies. Tablelamp itself is MIT licensed.</p>
<p>Each dependency has a link to its exact published source archive. MPL-2.0
components are unmodified; their source and license remain available through
those links. No dependency license is replaced by Tablelamp's MIT license.</p>
"""]
    sections.append(f"<!-- Cargo.lock SHA-256: {lock_hash} -->\n")
    crates = {}
    for index, license in enumerate(data["licenses"]):
        usages = [usage for usage in license["used_by"] if usage["crate"]["source"]]
        if not usages:
            continue
        sections.append(f'<details id="license-{index}"><summary>{escape(license["name"])}</summary><ul>\n')
        for usage in usages:
            crate = usage["crate"]
            name, version = crate["name"], crate["version"]
            crates[(name, version)] = crate
            source = f"https://crates.io/api/v1/crates/{name}/{version}/download"
            sections.append(f'<li>{escape(name)} {escape(version)} — <a href="{escape(source)}">source archive</a></li>\n')
        sections.append(f'</ul><pre>{escape(license["text"])}</pre></details>\n')

    # Include attribution files beyond SPDX license texts (notably Lucide/Feather).
    extras = []
    for (name, version), crate in sorted(crates.items()):
        if not crate["source"]:
            continue
        directory = pathlib.Path(crate["manifest_path"]).parent
        for file in sorted(directory.iterdir()):
            if file.is_file() and (file.name.upper().startswith("NOTICE") or
                                   (name == "gpui-kit-assets" and file.name == "LICENSE-LUCIDE")):
                extras.append((name, version, file))
    if not any(name == "gpui-kit-assets" and file.name == "LICENSE-LUCIDE" for name, _, file in extras):
        raise SystemExit("Missing embedded Lucide / Feather icon license")
    sections.append("<h2>Additional asset licenses and attribution</h2>\n")
    for name, version, file in extras:
        sections.append(f'<h3>{escape(name)} {escape(version)}: {escape(file.name)}</h3>\n<pre>{escape(file.read_text(encoding="utf-8"))}</pre>\n')
    sections.append("</html>\n")
    (ROOT / "THIRD_PARTY_NOTICES.html").write_text("".join(sections), encoding="utf-8")
    (ROOT / "packaging/licenses.lock.sha256").write_text(lock_hash + "\n", encoding="utf-8")
    print(f"Generated notices for {len(crates)} dependencies and {len(extras)} attribution files.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-about", default="cargo-about", help="Path to cargo-about 0.9.2")
    generate(parser.parse_args().cargo_about)
