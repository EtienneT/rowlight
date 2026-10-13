#!/usr/bin/env python3
"""Compose the social card from Tablelamp's existing logo and synthetic screenshot."""

import base64
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "docs" / "assets"


def data_url(name, mime):
    encoded = base64.b64encode((ASSETS / name).read_bytes()).decode("ascii")
    return f"data:{mime};base64,{encoded}"


def main():
    logo = data_url("logo-dark.svg", "image/svg+xml")
    screenshot = data_url("tokyo-night.png", "image/png")
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
  <title>Tablelamp — Shed light on your data.</title>
  <desc>A native viewer for Parquet, Arrow, CSV and TSV. Free and open source for Linux and Windows. The screenshot uses synthetic data.</desc>
  <defs>
    <linearGradient id="background" x2="1" y2="1"><stop stop-color="#171821"/><stop offset="1" stop-color="#222638"/></linearGradient>
    <linearGradient id="accent"><stop stop-color="#f3b85a"/><stop offset="1" stop-color="#ffe6b6"/></linearGradient>
    <clipPath id="window"><rect x="557" y="158" width="658" height="404" rx="13"/></clipPath>
  </defs>
  <rect width="1200" height="630" fill="url(#background)"/>
  <path d="M0 625H1200" stroke="url(#accent)" stroke-width="10"/>
  <image href="{logo}" x="50" y="30" width="280" height="84"/>
  <g font-family="Liberation Sans, Arial, sans-serif">
    <text x="55" y="219" fill="#eef0f6" font-size="66" font-weight="700" letter-spacing="-2">Shed light</text>
    <text x="55" y="296" fill="#f3b85a" font-size="66" font-weight="700" letter-spacing="-2">on your data.</text>
    <text x="58" y="357" fill="#c2c6d5" font-size="25">A native viewer for Parquet,</text>
    <text x="58" y="391" fill="#c2c6d5" font-size="25">Arrow, CSV and TSV.</text>
    <rect x="57" y="448" width="325" height="42" rx="21" fill="#303446"/>
    <text x="78" y="475" fill="#eef0f6" font-size="17">MIT licensed · Linux &amp; Windows</text>
    <text x="58" y="564" fill="#9ea6bc" font-size="19">etiennet.github.io/tablelamp</text>
  </g>
  <rect x="545" y="146" width="682" height="428" rx="22" fill="#101119"/>
  <image href="{screenshot}" x="557" y="158" width="658" height="404" clip-path="url(#window)"/>
  <rect x="557" y="158" width="658" height="404" rx="13" fill="none" stroke="#414860"/>
</svg>
'''
    source = ASSETS / "social-preview.svg"
    output = ASSETS / "social-preview.png"
    source.write_text(svg, encoding="utf-8")
    subprocess.run(["rsvg-convert", "--output", str(output), str(source)], check=True)
    print("Generated docs/assets/social-preview.svg and social-preview.png (1200 × 630).")


if __name__ == "__main__":
    main()
