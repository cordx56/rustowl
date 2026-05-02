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

    /// Compute a per-character `Style` for each character in `line`.
    ///
    /// The returned vector has one entry per `char` of `line` (not byte),
    /// so callers can compose it with character-indexed decorations.
    pub fn highlight_styles(&mut self, line: &str) -> Vec<Style> {
        // (byte_offset, char) so we can map tree-sitter byte ranges to char indices.
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        let mut styles = vec![Style::new(); chars.len()];

        let Ok(events) = self
            .highlighter
            .highlight(&self.config, line.as_bytes(), None, |_| None)
        else {
            return styles;
        };

        let mut style_stack: Vec<Style> = Vec::new();
        for event in events.flatten() {
            match event {
                HighlightEvent::Source { start, end } => {
                    let style = style_stack.last().copied().unwrap_or_default();
                    for (i, (bi, _)) in chars.iter().enumerate() {
                        if *bi >= start && *bi < end {
                            styles[i] = style;
                        }
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

        styles
    }
}

/// Compute a per-character `Style` for each character in `line`.
///
/// Convenience wrapper using a thread-local highlighter; for many lines,
/// instantiate [`Highlighter`] once and reuse it.
pub fn highlight_styles(line: &str) -> Vec<Style> {
    thread_local! {
        static HIGHLIGHTER: std::cell::RefCell<Highlighter> = std::cell::RefCell::new(Highlighter::new());
    }

    HIGHLIGHTER.with(|h| h.borrow_mut().highlight_styles(line))
}
