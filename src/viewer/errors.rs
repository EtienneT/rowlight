use super::*;

struct Explanation {
    message: String,
    hint: &'static str,
}

impl Explanation {
    fn new(message: impl Into<String>, hint: &'static str) -> Self {
        Self {
            message: message.into(),
            hint,
        }
    }
}

fn short_text(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

// Keep the original diagnostic intact for Details and Copy details. Match only
// recognizable causes; an unfamiliar engine error gets an honest generic hint.
fn explain(error: &str) -> Explanation {
    let lower = error.to_lowercase();
    if let Some(rest) = error.split_once("unable to find column \"").map(|(_, s)| s)
        && let Some((name, _)) = rest.split_once('"')
    {
        return Explanation::new(
            format!("Column “{}” was not found.", short_text(name, 80)),
            "Check the source columns. Use 'text' for text values; \"name\" refers to a column.",
        );
    }
    if lower.contains("relation '") && lower.contains("was not found") {
        return Explanation::new(
            "That table was not found.",
            "The opened file or folder is available as the table data. Use FROM data.",
        );
    }
    if lower.contains("sql parser error") || lower.contains("sql syntax") {
        return Explanation::new(
            "The SQL query has a syntax error.",
            "Check the spelling, quotes and parentheses. Details shows where parsing failed.",
        );
    }
    if lower.contains("memory budget")
        || lower.contains("safe decoding budget")
        || lower.contains("out of memory")
    {
        return Explanation::new(
            "There isn't enough memory to safely finish this request.",
            "Use fewer rows or columns. For SQL, narrow the query or add LIMIT.",
        );
    }
    if lower.contains("temporary-storage budget")
        || lower.contains("temporary-file limit")
        || lower.contains("no space left on device")
        || lower.contains("disk full")
    {
        return Explanation::new(
            "There isn't enough temporary storage for this request.",
            "Free some disk space, or narrow the query and add LIMIT.",
        );
    }
    if lower.contains("permission denied") || lower.contains("access is denied") {
        return Explanation::new(
            "Rowlight doesn't have permission to access this file or folder.",
            "Check its access permissions, then try opening it again.",
        );
    }
    if lower.contains("no such file or directory")
        || lower.contains("(os error 2)")
        || lower.contains("(os error 3)")
    {
        return Explanation::new(
            "The file or folder could not be found.",
            "It may have moved or been deleted. Open it from its current location.",
        );
    }
    if lower.contains("timed out") || lower.contains("cannot reach remote object") {
        return Explanation::new(
            "Rowlight couldn't reach the remote file.",
            "Check your connection and the file URL, then try again.",
        );
    }
    if lower.contains("cannot configure cloud source")
        || lower.contains("cannot read the aws shared credential profile")
        || lower.contains("aws profile") && lower.contains("not found")
    {
        return Explanation::new(
            "The cloud connection could not be configured.",
            "Check the provider URL, credentials and selected profile.",
        );
    }
    if lower.contains("changed while") || lower.contains("remote object changed") {
        return Explanation::new(
            "The source changed while Rowlight was reading it.",
            "Reopen the file or folder to read its current version.",
        );
    }
    if lower.contains("parquet schemas differ") {
        return Explanation::new(
            "The folder contains files with different columns or data types.",
            "Open a single file, or choose a folder whose Parquet files share the same schema.",
        );
    }
    if lower.contains("invalid data file") || lower.contains("invalid parquet") {
        return Explanation::new(
            "This data file couldn't be read.",
            "Check that it is complete and uses a supported format. If it was downloaded, try downloading it again.",
        );
    }
    if lower.contains("sql failed") || lower.contains("sql worker stopped") {
        return Explanation::new(
            "The query could not be run.",
            "Check your query and the source columns. Details contains the full error.",
        );
    }
    Explanation::new(
        short_text(
            error
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Something went wrong."),
            200,
        ),
        "Open Details for more information.",
    )
}

impl Viewer {
    pub(super) fn error_banner(
        &self,
        error: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let explanation = explain(error);
        let details_height = (window.viewport_size().height * 0.12).min(px(96.0));
        div()
            .id("error-banner")
            .test_support()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .min_w_0()
            .gap_1()
            .p_2()
            .bg(cx.omarchy().danger.opacity(0.1))
            .text_color(cx.omarchy().danger)
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .min_w_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .line_clamp(2)
                            .child(explanation.message),
                    )
                    .child(
                        control(
                            "error-details-toggle",
                            if self.error_details {
                                "Hide details"
                            } else {
                                "Details"
                            },
                            cx,
                        )
                        .flex_shrink_0()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.error_details = !this.error_details;
                            this.error_scroll.set_offset(point(px(0.0), px(0.0)));
                            cx.notify();
                        })),
                    )
                    .child(
                        control("error-copy", "Copy details", cx)
                            .flex_shrink_0()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(error) = &this.error {
                                    cx.write_to_clipboard(ClipboardItem::new_string(error.clone()));
                                }
                            })),
                    )
                    .child(control("dismiss", "Dismiss", cx).flex_shrink_0().on_click(
                        cx.listener(|this, _, _, cx| {
                            this.error = None;
                            this.error_details = false;
                            cx.notify();
                        }),
                    )),
            )
            .child(div().line_clamp(2).child(explanation.hint))
            .when(self.error_details, |d| {
                d.child(
                    div()
                        .relative()
                        .h(details_height)
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            div()
                                .id("error-diagnostic")
                                .test_support()
                                .size_full()
                                .overflow_scroll()
                                .track_scroll(&self.error_scroll)
                                .child(
                                    div()
                                        .pr_4()
                                        .pb_3()
                                        .whitespace_nowrap()
                                        .font_family("monospace")
                                        .child(error.to_owned()),
                                ),
                        )
                        .child(scrollbar(
                            "error-scrollbar",
                            ScrollbarAxis::Vertical,
                            &self.error_scroll,
                            window,
                            cx,
                        ))
                        .child(scrollbar(
                            "error-horizontal-scrollbar",
                            ScrollbarAxis::Horizontal,
                            &self.error_scroll,
                            window,
                            cx,
                        )),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::explain;

    #[test]
    fn common_causes_have_actionable_messages_without_engine_plans() {
        for (error, message, hint) in [
            (
                "SQL failed: Error: not found: unable to find column \"AECI\"; valid columns: [\"node\"]\nResolved plan until failure:\nPARQUET SCAN",
                "Column “AECI” was not found.",
                "Use 'text'",
            ),
            (
                "SQL failed: relation 'other' was not found",
                "That table was not found.",
                "FROM data",
            ),
            (
                "sql parser error: Expected: ), found: EOF",
                "The SQL query has a syntax error.",
                "parentheses",
            ),
            (
                "SQL exceeded its 256 MiB memory budget",
                "There isn't enough memory to safely finish this request.",
                "LIMIT",
            ),
            (
                "SQL exceeded its 4 GiB temporary-storage budget",
                "There isn't enough temporary storage for this request.",
                "disk space",
            ),
            (
                "Cannot open file: Permission denied (os error 13)",
                "Rowlight doesn't have permission to access this file or folder.",
                "permissions",
            ),
            (
                "Cannot open file: No such file or directory (os error 2)",
                "The file or folder could not be found.",
                "current location",
            ),
            (
                "Remote request timed out after 30 seconds",
                "Rowlight couldn't reach the remote file.",
                "connection",
            ),
            (
                "Parquet schemas differ in part.parquet",
                "The folder contains files with different columns or data types.",
                "single file",
            ),
            (
                "SQL failed: unfamiliar engine failure\nResolved plan until failure:",
                "The query could not be run.",
                "Details",
            ),
        ] {
            let explanation = explain(error);
            assert_eq!(explanation.message, message, "{error}");
            assert!(explanation.hint.contains(hint), "{error}");
        }
    }

    #[test]
    fn unfamiliar_errors_and_untrusted_names_are_bounded_and_unicode_safe() {
        let error = format!("{}\nInternal plan", "é".repeat(1000));
        assert_eq!(explain(&error).message.chars().count(), 201);
        let error = format!("unable to find column \"{}\"", "列\n".repeat(1000));
        let message = explain(&error).message;
        assert!(!message.contains('\n'));
        assert!(message.chars().count() < 120);
        assert_eq!(
            explain("Select at most 10,000 cells to copy").message,
            "Select at most 10,000 cells to copy"
        );
        assert_eq!(
            explain("Not a directory (os error 20)").message,
            "Not a directory (os error 20)"
        );
        assert_eq!(
            explain("SQL failed: unsupported operation\nResolved plan: credentials").message,
            "The query could not be run."
        );
    }
}
