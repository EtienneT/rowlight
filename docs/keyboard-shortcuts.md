# Keyboard shortcuts

[User guide](user-guide.md) · [Installation](installation.md) · [Formats](formats.md)

Press **F1**, or choose **Keyboard shortcuts…** from the top-left menu, to see the reference inside Rowlight.

## Everyday actions

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| Open a file | Ctrl+O | Cmd+O |
| Open a Hive Parquet folder | Ctrl+Shift+O | Cmd+Shift+O |
| Open the SQL editor | Ctrl+Shift+L | Cmd+Shift+L |
| Run SQL, while its editor is open | Ctrl+Enter | Cmd+Enter |
| Find a column by name or type | Ctrl+F | Cmd+F |
| Focus the page number; type a number and press Enter | Ctrl+G | Cmd+G |
| Inspect the selected cell | Enter | Enter |
| Clear selection or return from a chart/tool | Esc | Esc |
| Show shortcut reference | F1 | F1 |
| Quit | Ctrl+Q | Cmd+Q |

## Move within the loaded page

| Destination | Linux / Windows | macOS |
| --- | --- | --- |
| Adjacent cell | Arrow keys | Arrow keys |
| First / last row of the current page | Home / End | Home / End |
| One screenful up / down | Page Up / Page Down | Page Up / Page Down |
| First / last visible column | Ctrl+Left / Right | Cmd+Left / Right |

Add **Shift** to these navigation shortcuts to extend the selection. **Shift+click** also extends a rectangular selection. Clicking the source-row gutter selects a whole row.

## Move between file pages

| Destination | Linux / Windows | macOS |
| --- | --- | --- |
| Previous / next page | Ctrl+Page Up / Page Down | Cmd+Page Up / Page Down |
| Previous / next page, alternative | Alt+Left / Right | Option+Left / Right |
| First row of the first page / last row of the last page | Ctrl+Home / End | Cmd+Home / End |
| Same, macOS alternative | — | Cmd+Up / Down |

File-page navigation follows active filters or the active SQL result. Previous/next page keeps the selected column. These shortcuts use ordinary paging and never switch to **All** mode.

## Select and copy

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| Select all loaded rows and visible columns | Ctrl+A | Cmd+A |
| Copy selected cells | Ctrl+C | Cmd+C |
| Copy selected rows with visible column names | Ctrl+Shift+C | Cmd+Shift+C |

Selections stay within the loaded page. **Select all** does not load more rows or bypass the **10,000-cell / 1 MiB** copy limits. See [Inspect and copy cells](user-guide.md#inspect-and-copy-cells) for output formatting.

Table shortcuts work immediately after opening a file. When a text input is focused, its normal editing shortcuts take precedence; return to the table to navigate cells.
