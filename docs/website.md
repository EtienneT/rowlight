# Rowlight website

The landing page lives in [index.html](index.html), with [site.css](site.css) and
[site.js](site.js). It is plain static HTML, CSS, and JavaScript: no dependencies,
build step, external fonts, analytics, or server application. Existing synthetic
Linux app captures in `assets/` are shared with the documentation.

The [user guide](guide.html), [installation instructions](installation.html),
[formats and limits](formats.html), and [keyboard shortcuts](keyboard-shortcuts.html)
are also served on the website. Shared `theme.js` keeps the selected website
palette consistent across the landing page and documentation. `guide.css`
provides the reading layout, section navigation, and scrollable reference tables.

## Update the website documentation

Edit the existing Markdown files (`user-guide.md`, `installation.md`, `formats.md`,
and `keyboard-shortcuts.md`), then regenerate their checked-in HTML:

```sh
uv run --script scripts/build-website-docs.py
uv run --script scripts/build-website-docs.py --check
```

The generator pins Python-Markdown in uv's isolated script cache; it does not
install a global package. GitHub Pages serves the generated HTML directly and
does not run a documentation build. Include the generated files in the same
commit as source changes. The shared page template lives in
`scripts/templates/website-docs.html`; the header and footer come from `index.html`.
Regenerate after changes to their navigation as well.

## Preview locally

From the repository root, run:

```sh
python3 -m http.server 4173 --bind 127.0.0.1 --directory docs
```

Open <http://127.0.0.1:4173/>. Reload after edits; Ctrl+C stops the server.
Binding to loopback keeps the preview on your machine.

The web illustration has 32 synthetic rows and eight rows per page. Visitors can
sort the displayed page, inspect cells, expand distributions, and hide plots.
It does not read local files or execute SQL. The theme picker changes the website
and selects a real app screenshot; it remembers only the website palette in
browser-local storage under `rowlight-site-theme`. Storage is optional.

Without JavaScript, the hero shows a real app screenshot and the content,
documentation links, release links, and FAQ remain usable. Direct screenshot
links provide the other theme captures.

## GitHub Pages

All local asset URLs are relative, so the same source can serve under the project
path `https://etiennet.github.io/rowlight/` or a custom domain. `docs/.nojekyll`
disables Jekyll processing. There is no custom domain configured in this change.

[GitHub supports publishing from the `/docs` folder of a branch](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site).
The publishing source is **main → /docs**, using **Deploy from a branch**.
Pushes to `main` publish the source automatically. The public URL is
<https://etiennet.github.io/rowlight/>. No custom build workflow is needed.

GitHub Pages still uses a GitHub Actions deployment run with branch publishing.
The owner authorized website publication on 2026-10-13. This authorization covers
Pages deployment; the existing **Native builds** workflow stays disabled.
The repository's instructions still require explicit authorization for enabling
or dispatching application build/test workflows.

## Keep the page current

- The page links to the published `v0.1.0-alpha.1` release and its Linux archive,
  Arch/Omarchy package, Windows installer, portable ZIP, and `SHA256SUMS`.
  Update these links and the release notice together when a new release ships.
  Keep Linux x64 and Windows x64 as the official downloads and retain requirements.
- Keep the unsigned-alpha notice accurate. Checksums verify integrity, not
  publisher identity. Do not add package-manager commands before packages exist.
- Preserve the distinction between page charts and the sampled file overview;
  column sorting is page-local unless All mode or SQL is used.
- Replace app screenshots only with captures using synthetic data. Keep their
  descriptions accurate; the gallery demonstrates Linux theme integration.
- Check `node --check docs/site.js`, `node --check docs/theme.js`,
  `uv run --script scripts/build-website-docs.py --check`, `git diff --check`, and the rendered desktop
  and narrow layouts after changes. Exercise sorting, pagination, inspection,
  theme switching, and keyboard dismissal of the inspector.
- Keep navigation, content, and screenshots usable without JavaScript. Avoid
  third-party runtime requests and retain reduced-motion support.

The layout takes inspiration from [Strata](https://stratafiles.io/): introduce the
product through a usable illustration, show its appearance, and give visitors a
clear next step. Rowlight's copy, styling, demo code, and brand assets are its own.
