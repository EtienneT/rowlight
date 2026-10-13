# /// script
# requires-python = ">=3.10"
# dependencies = ["Markdown==3.11"]
# ///
"""Render checked-in website documentation from the existing Markdown files.

Run with: uv run --script scripts/build-website-docs.py [--check]
The dependency stays in uv's script cache; GitHub Pages serves static HTML.
"""

import argparse
import html
import posixpath
import re
import struct
from html.parser import HTMLParser
from pathlib import Path
from string import Template
from urllib.parse import urlsplit

import markdown

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
REPO = "https://github.com/EtienneT/rowlight"
PAGES = {
    "user-guide.md": (
        "guide.html",
        "User guide",
        "Open files, explore columns, inspect cells, filter data, and query with SQL in Rowlight.",
    ),
    "installation.md": (
        "installation.html",
        "Installation",
        "Download and install Rowlight on Linux and Windows, verify checksums, and configure file associations.",
    ),
    "formats.md": (
        "formats.html",
        "Formats & limits",
        "Supported Rowlight data sources, paging, SQL, remote Parquet, sampling, and memory limits.",
    ),
    "keyboard-shortcuts.md": (
        "keyboard-shortcuts.html",
        "Keyboard shortcuts",
        "Navigate, select, copy, inspect, and query data with Rowlight's keyboard shortcuts.",
    ),
}


def rewrite_link(url, home_anchors=False):
    parsed = urlsplit(url)
    if parsed.scheme or parsed.netloc:
        return url
    if home_anchors and not parsed.path:
        return "index.html" + (f"#{parsed.fragment}" if parsed.fragment else "")
    if parsed.path == "../README.md":
        return (
            "index.html#themes"
            if parsed.fragment == "your-omarchy-theme"
            else "index.html"
        )
    if parsed.path in PAGES:
        return PAGES[parsed.path][0] + (
            f"#{parsed.fragment}" if parsed.fragment else ""
        )
    if parsed.path.endswith(".md"):
        path = posixpath.normpath("docs/" + parsed.path)
        return f"{REPO}/blob/main/{path}" + (
            f"#{parsed.fragment}" if parsed.fragment else ""
        )
    return url


class WebsiteHTML(HTMLParser):
    def __init__(self, *, home_anchors=False, content=False):
        super().__init__(convert_charrefs=False)
        self.parts = []
        self.home_anchors = home_anchors
        self.content = content

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "href" in attrs:
            attrs["href"] = rewrite_link(attrs["href"], self.home_anchors)
        if self.home_anchors and tag == "a" and attrs.get("href") == "guide.html":
            attrs["aria-current"] = "location"
        if self.content and tag == "img":
            attrs["loading"] = "lazy"
            image = DOCS / attrs["src"]
            if image.suffix == ".png":
                with image.open("rb") as source:
                    header = source.read(24)
                width, height = struct.unpack(">II", header[16:24])
                attrs.update(width=str(width), height=str(height))
        if self.content and tag == "pre":
            attrs["tabindex"] = "0"
        if self.content and tag == "table":
            self.parts.append(
                '<div class="guide-table-scroll" tabindex="0" role="region" aria-label="Scrollable reference table">'
            )
        attributes = "".join(
            f' {key}="{html.escape(value, quote=True)}"'
            if value is not None
            else f" {key}"
            for key, value in attrs.items()
        )
        self.parts.append(f"<{tag}{attributes}>")

    def handle_endtag(self, tag):
        self.parts.append(f"</{tag}>")
        if self.content and tag == "table":
            self.parts.append("</div>")

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in {
            "area",
            "base",
            "br",
            "col",
            "embed",
            "hr",
            "img",
            "input",
            "link",
            "meta",
            "param",
            "source",
            "track",
            "wbr",
        }:
            self.handle_endtag(tag)

    def handle_data(self, data):
        self.parts.append(data)

    def handle_entityref(self, name):
        self.parts.append(f"&{name};")

    def handle_charref(self, name):
        self.parts.append(f"&#{name};")


def website_html(source, **options):
    parser = WebsiteHTML(**options)
    parser.feed(source)
    parser.close()
    return "".join(parser.parts)


def section_tokens(tokens):
    for token in tokens:
        if token["level"] == 2:
            yield token
        yield from section_tokens(token["children"])


def heading_slug(value, separator):
    # Preserve the GitHub-style anchors already linked from the Markdown files.
    return re.sub(r"[^\w\- ]", "", value.strip().lower()).replace(" ", separator)


def installation_tabs(body):
    linux = re.search(r'<h2 id="linux">.*?(?=<h2 id="windows">)', body, re.DOTALL)
    windows = re.search(
        r'<h2 id="windows">.*?(?=<h2 id="download-checksums">)', body, re.DOTALL
    )
    if linux is None or windows is None:
        raise ValueError(
            "Installation source must contain Linux, Windows, and Download checksums sections."
        )
    tabs = '<nav class="install-tabs" aria-label="Operating system"><a id="install-tab-linux" href="#linux">Linux</a><a id="install-tab-windows" href="#windows">Windows</a></nav>'
    panels = (
        '<section class="install-panel" id="install-linux">'
        + linux[0]
        + '</section><section class="install-panel" id="install-windows">'
        + windows[0]
        + "</section>"
    )
    return body[: linux.start()] + tabs + panels + body[windows.end() :]


def render_pages():
    homepage = (DOCS / "index.html").read_text(encoding="utf-8")
    header = website_html(
        re.search(r'<header class="site-header">.*?</header>', homepage, re.DOTALL)[0],
        home_anchors=True,
    )
    footer = website_html(
        re.search(r"<footer\b.*?</footer>", homepage, re.DOTALL)[0], home_anchors=True
    )
    template = Template(
        (ROOT / "scripts/templates/website-docs.html").read_text(encoding="utf-8")
    )
    for source_name, (filename, title, description) in PAGES.items():
        source = (DOCS / source_name).read_text(encoding="utf-8")
        source = source.replace("<details>", '<details markdown="1">')
        source = re.sub(
            r"> \[!(NOTE|WARNING|TIP|IMPORTANT)\]",
            lambda match: f"> **{match[1].title()}:**",
            source,
        )
        renderer = markdown.Markdown(
            extensions=["tables", "fenced_code", "toc", "md_in_html", "sane_lists"],
            extension_configs={"toc": {"slugify": heading_slug}},
        )
        body = renderer.convert(source)
        body = re.sub(r"^<h1\b.*?</h1>\s*", "", body, count=1, flags=re.DOTALL)
        body = website_html(body, content=True)
        if source_name == "installation.md":
            body = installation_tabs(body)
        toc = (
            "<ul>"
            + "".join(
                f'<li><a href="#{token["id"]}">{html.escape(token["name"])}</a></li>'
                for token in section_tokens(renderer.toc_tokens)
            )
            + "</ul>"
        )
        doc_links = "".join(
            f'<a href="{route}"'
            + (' aria-current="page"' if route == filename else "")
            + f">{html.escape(label)}</a>"
            for route, label, _ in PAGES.values()
        )
        yield (
            DOCS / filename,
            template.substitute(
                title=html.escape(title),
                description=html.escape(description, quote=True),
                header=header,
                footer=footer,
                doc_links=doc_links,
                toc=toc,
                body=body,
                source_url=f"{REPO}/blob/main/docs/{source_name}",
                page_scripts='    <script src="install.js" defer></script>'
                if source_name == "installation.md"
                else "",
            ),
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check that generated HTML is current without writing it.",
    )
    args = parser.parse_args()
    stale = []
    for path, rendered in render_pages():
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != rendered:
                stale.append(path.name)
        else:
            path.write_text(rendered, encoding="utf-8", newline="\n")
            print(f"Rendered docs/{path.name}")
    if stale:
        parser.exit(1, "Regenerate website documentation: " + ", ".join(stale) + "\n")
    if args.check:
        print("All four website documentation pages are current.")


if __name__ == "__main__":
    main()
