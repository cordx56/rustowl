//! Rust syntax highlighter using tree-sitter-highlight.

use crate::visualize::colors;
use anstyle::Style;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};

/// Recognized highlight names from tree-sitter-rust's highlights.scm.
/// The order matters - it must match the order passed to `configure`.
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constructor",
    "escape",
    "function",
    "function.macro",
    "function.method",
    "keyword",
    "label",
    "operator",
    "property",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "type",
    "type.builtin",
    "variable.builtin",
    "variable.parameter",
];

/// Map highlight index to style.
fn style_for_highlight(index: usize) -> Option<Style> {
    match HIGHLIGHT_NAMES.get(index)? {
        &"attribute" => Some(colors::ATTRIBUTE),
        &"comment" | &"comment.documentation" => Some(colors::COMMENT),
        &"constant" | &"constant.builtin" => Some(colors::NUMBER),
        &"constructor" => Some(colors::TYPE),
        &"escape" => Some(colors::STRING),
        &"function" | &"function.method" => Some(colors::MACRO),
        &"function.macro" => Some(colors::MACRO),
        &"keyword" => Some(colors::KEYWORD),
        &"label" => Some(colors::LIFETIME),
        &"operator" => Some(colors::KEYWORD),
        &"property" => Some(colors::CYAN),
        &"punctuation.bracket" | &"punctuation.delimiter" => Some(colors::DIM),
        &"string" => Some(colors::STRING),
        &"type" | &"type.builtin" => Some(colors::TYPE),
        &"variable.builtin" => Some(colors::KEYWORD),
        &"variable.parameter" => Some(colors::CYAN),
        _ => None,
    }
}

/// Syntax highlighter using tree-sitter-highlight for Rust.
pub struct Highlighter {
    highlighter: TsHighlighter,
    config: HighlightConfiguration,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    pub fn new() -> Self {
        let mut config = HighlightConfiguration::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        )
        .expect("Error loading Rust highlight configuration");

        config.configure(HIGHLIGHT_NAMES);

        Self {
            highlighter: TsHighlighter::new(),
            config,
        }
    }

    /// Highlight a line of Rust code.
    pub fn highlight(&mut self, line: &str) -> String {
        use std::fmt::Write;

        let Ok(events) = self
            .highlighter
            .highlight(&self.config, line.as_bytes(), None, |_| None)
        else {
            return line.to_string();
        };

        let mut result = String::with_capacity(line.len() * 2);
        let mut style_stack: Vec<Style> = Vec::new();

        for event in events.flatten() {
            match event {
                HighlightEvent::Source { start, end } => {
                    let text = &line[start..end];
                    if let Some(&style) = style_stack.last() {
                        let _ = write!(result, "{style}{text}{style:#}");
                    } else {
                        result.push_str(text);
                    }
                }
                HighlightEvent::HighlightStart(highlight) => {
                    let style = style_for_highlight(highlight.0).unwrap_or_default();
                    style_stack.push(style);
                }
                HighlightEvent::HighlightEnd => {
                    style_stack.pop();
                }
            }
        }

        result
    }
}

/// Highlight a line of Rust code.
///
/// This is a convenience function that uses a thread-local highlighter.
/// For better performance when highlighting multiple lines, use [`Highlighter`] directly.
pub fn highlight(line: &str) -> String {
    thread_local! {
        static HIGHLIGHTER: std::cell::RefCell<Highlighter> = std::cell::RefCell::new(Highlighter::new());
    }

    HIGHLIGHTER.with(|h| h.borrow_mut().highlight(line))
}
