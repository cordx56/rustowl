//! CLI visualization module for ownership and lifetime display.
//!
//! This module provides terminal-based visualization of Rust ownership
//! and lifetime information, using colored underlines to represent
//! different ownership states.

use crate::lsp::decoration::{CalcDecos, Deco};
use crate::models::*;
use crate::utils::{self, MirVisitor};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// ANSI color codes for different decoration types and syntax highlighting.
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[92m";
    pub const CYAN: &str = "\x1b[96m";
    pub const PURPLE: &str = "\x1b[38;5;177m";
    pub const MAGENTA: &str = "\x1b[95m";
    pub const YELLOW: &str = "\x1b[93m";
    pub const RED: &str = "\x1b[91m";

    pub const DIM: &str = "\x1b[2m";

    // Syntax highlighting colors
    pub const KEYWORD: &str = MAGENTA;
    pub const TYPE: &str = YELLOW;
    pub const STRING: &str = GREEN;
    pub const NUMBER: &str = CYAN;
    pub const COMMENT: &str = "\x1b[90m";
    pub const LIFETIME: &str = YELLOW;
    pub const MACRO: &str = CYAN;
    pub const ATTRIBUTE: &str = MAGENTA;
}

/// Simple Rust syntax highlighter using a state-machine based lexer.
mod syntax {
    use super::colors;

    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "yield",
    ];

    const PRIMITIVE_TYPES: &[&str] = &[
        "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32",
        "i64", "i128", "isize", "f32", "f64",
    ];

    /// Highlight a line of Rust code.
    pub fn highlight(line: &str) -> String {
        // Handle line comments
        if let Some(comment_start) = find_line_comment(line) {
            let (code, comment) = line.split_at(comment_start);
            let highlighted_code = highlight_code(code);
            return format!(
                "{}{}{}{}",
                highlighted_code,
                colors::COMMENT,
                comment,
                colors::RESET
            );
        }

        highlight_code(line)
    }

    /// Find the start of a line comment, accounting for strings.
    fn find_line_comment(line: &str) -> Option<usize> {
        let mut in_string = false;
        let mut in_char = false;
        let mut escape_next = false;
        let chars: Vec<char> = line.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }

            if ch == '\\' && (in_string || in_char) {
                escape_next = true;
                continue;
            }

            if !in_char && ch == '"' {
                in_string = !in_string;
            } else if !in_string && ch == '\'' {
                // Check if this could be a lifetime (not a char literal start)
                if !in_char && i + 1 < chars.len() {
                    let next = chars[i + 1];
                    if next.is_alphabetic() || next == '_' {
                        // Could be lifetime or char, need to check further
                        let mut j = i + 2;
                        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                            j += 1;
                        }
                        if j < chars.len() && chars[j] == '\'' {
                            // It's a char literal, skip to end
                            in_char = true;
                        }
                        // Otherwise it's a lifetime, don't set in_char
                    } else {
                        in_char = true;
                    }
                }
            } else if in_char && ch == '\'' {
                in_char = false;
            }

            if !in_string && !in_char && ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                return Some(i);
            }
        }

        None
    }

    fn highlight_code(code: &str) -> String {
        let mut result = String::with_capacity(code.len() * 2);
        let chars: Vec<char> = code.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            // Check for raw strings r#"..."#
            if chars[i] == 'r' && i + 1 < len {
                let mut hash_count = 0;
                let mut j = i + 1;
                while j < len && chars[j] == '#' {
                    hash_count += 1;
                    j += 1;
                }
                if j < len && chars[j] == '"' {
                    // Raw string literal
                    result.push_str(colors::STRING);
                    result.push('r');
                    for _ in 0..hash_count {
                        result.push('#');
                    }
                    result.push('"');
                    j += 1;
                    // Find closing "###
                    while j < len {
                        result.push(chars[j]);
                        if chars[j] == '"' {
                            let mut closing_hashes = 0;
                            let mut k = j + 1;
                            while k < len && chars[k] == '#' && closing_hashes < hash_count {
                                closing_hashes += 1;
                                k += 1;
                            }
                            if closing_hashes == hash_count {
                                for _ in 0..hash_count {
                                    result.push('#');
                                }
                                j = k;
                                break;
                            }
                        }
                        j += 1;
                    }
                    result.push_str(colors::RESET);
                    i = j;
                    continue;
                }
            }

            // Check for byte strings b"..." and byte chars b'...'
            if chars[i] == 'b' && i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '\'') {
                let quote = chars[i + 1];
                result.push_str(colors::STRING);
                result.push('b');
                result.push(quote);
                i += 2;
                while i < len {
                    if chars[i] == '\\' && i + 1 < len {
                        result.push(chars[i]);
                        result.push(chars[i + 1]);
                        i += 2;
                    } else if chars[i] == quote {
                        result.push(chars[i]);
                        i += 1;
                        break;
                    } else {
                        result.push(chars[i]);
                        i += 1;
                    }
                }
                result.push_str(colors::RESET);
                continue;
            }

            // Check for attributes #[...] or #![...]
            if chars[i] == '#'
                && i + 1 < len
                && (chars[i + 1] == '['
                    || (chars[i + 1] == '!' && i + 2 < len && chars[i + 2] == '['))
            {
                result.push_str(colors::ATTRIBUTE);
                result.push(chars[i]);
                i += 1;
                if chars[i] == '!' {
                    result.push(chars[i]);
                    i += 1;
                }
                let mut depth = 0;
                while i < len {
                    if chars[i] == '[' {
                        depth += 1;
                    } else if chars[i] == ']' {
                        depth -= 1;
                    }
                    result.push(chars[i]);
                    i += 1;
                    if depth == 0 {
                        break;
                    }
                }
                result.push_str(colors::RESET);
                continue;
            }

            // Check for strings
            if chars[i] == '"' {
                result.push_str(colors::STRING);
                result.push(chars[i]);
                i += 1;
                while i < len {
                    if chars[i] == '\\' && i + 1 < len {
                        result.push(chars[i]);
                        result.push(chars[i + 1]);
                        i += 2;
                    } else if chars[i] == '"' {
                        result.push(chars[i]);
                        i += 1;
                        break;
                    } else {
                        result.push(chars[i]);
                        i += 1;
                    }
                }
                result.push_str(colors::RESET);
                continue;
            }

            // Check for char literals and lifetimes
            if chars[i] == '\'' {
                let start = i;
                i += 1;

                if i < len && (chars[i].is_alphabetic() || chars[i] == '_') {
                    // Could be lifetime or char literal
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }

                    if i < len && chars[i] == '\'' {
                        // Char literal like 'a' or '\n'
                        result.push_str(colors::STRING);
                        for c in &chars[start..=i] {
                            result.push(*c);
                        }
                        result.push_str(colors::RESET);
                        i += 1;
                    } else {
                        // Lifetime like 'a
                        result.push_str(colors::LIFETIME);
                        for c in &chars[start..i] {
                            result.push(*c);
                        }
                        result.push_str(colors::RESET);
                    }
                } else if i < len && chars[i] == '\\' {
                    // Escaped char literal like '\n'
                    result.push_str(colors::STRING);
                    result.push('\'');
                    while i < len && chars[i] != '\'' {
                        result.push(chars[i]);
                        i += 1;
                    }
                    if i < len {
                        result.push(chars[i]);
                        i += 1;
                    }
                    result.push_str(colors::RESET);
                } else {
                    // Just a quote
                    result.push('\'');
                }
                continue;
            }

            // Check for numbers (including hex, binary, octal)
            if chars[i].is_ascii_digit()
                || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit())
            {
                result.push_str(colors::NUMBER);

                // Handle 0x, 0b, 0o prefixes
                if chars[i] == '0' && i + 1 < len {
                    match chars[i + 1] {
                        'x' | 'X' | 'b' | 'B' | 'o' | 'O' => {
                            result.push(chars[i]);
                            result.push(chars[i + 1]);
                            i += 2;
                        }
                        _ => {}
                    }
                }

                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    result.push(chars[i]);
                    i += 1;
                }
                result.push_str(colors::RESET);
                continue;
            }

            // Check for identifiers (keywords, types, macros)
            if chars[i].is_alphabetic() || chars[i] == '_' {
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                // Check for macro invocation
                if i < len && chars[i] == '!' {
                    result.push_str(colors::MACRO);
                    result.push_str(&word);
                    result.push('!');
                    result.push_str(colors::RESET);
                    i += 1;
                } else if KEYWORDS.contains(&word.as_str()) {
                    result.push_str(colors::KEYWORD);
                    result.push_str(&word);
                    result.push_str(colors::RESET);
                } else if PRIMITIVE_TYPES.contains(&word.as_str())
                    || word.chars().next().is_some_and(|c| c.is_uppercase())
                {
                    result.push_str(colors::TYPE);
                    result.push_str(&word);
                    result.push_str(colors::RESET);
                } else {
                    result.push_str(&word);
                }
                continue;
            }

            // Default: just push the character
            result.push(chars[i]);
            i += 1;
        }

        result
    }
}

/// Error types for visualization operations.
#[derive(Debug)]
pub enum VisualizeError {
    FileNotFound(String),
    FunctionNotFound(String),
    VariableNotFound(String),
    SourceReadError(std::io::Error),
}

impl fmt::Display for VisualizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisualizeError::FileNotFound(path) => write!(f, "File not found: {path}"),
            VisualizeError::FunctionNotFound(name) => write!(f, "Function not found: {name}"),
            VisualizeError::VariableNotFound(name) => write!(f, "Variable not found: {name}"),
            VisualizeError::SourceReadError(e) => write!(f, "Failed to read source file: {e}"),
        }
    }
}

impl std::error::Error for VisualizeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VisualizeError::SourceReadError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VisualizeError {
    fn from(e: std::io::Error) -> Self {
        VisualizeError::SourceReadError(e)
    }
}

/// Information about a found variable.
#[derive(Debug, Clone)]
pub struct VariableInfo {
    pub local: FnLocal,
    pub name: String,
    pub span: Range,
    pub function_name: String,
}

/// Find variables by name within a specific function.
struct FindVariablesByName<'a> {
    function_path: &'a str,
    variable_name: &'a str,
    current_function_name: String,
    found: Vec<VariableInfo>,
}

impl<'a> FindVariablesByName<'a> {
    fn new(function_path: &'a str, variable_name: &'a str) -> Self {
        Self {
            function_path,
            variable_name,
            current_function_name: String::new(),
            found: Vec::new(),
        }
    }

    /// Check if the function name matches the given path.
    ///
    /// The function path can be:
    /// - A simple function name: `foo` matches `crate::module::foo`
    /// - A partial path: `module::foo` matches `crate::module::foo`
    /// - A full path: `crate::module::foo` matches exactly
    /// - Async functions: `foo` matches `crate::module::foo::{closure#0}` (async state machine)
    fn matches_function(&self, name: &str) -> bool {
        // Strip async/closure suffixes from the function name
        // Async functions are compiled to state machines with names like:
        // - `func::{closure#0}`
        // - `func::{async_block#0}`
        // - `func::{async fn body}`
        let base_name = Self::strip_async_suffix(name);

        // Exact match
        if base_name == self.function_path {
            return true;
        }

        // Check if the function name ends with the given path
        // e.g., "module::foo" matches "crate::module::foo"
        if base_name.ends_with(&format!("::{}", self.function_path)) {
            return true;
        }

        // Check if the given path is a suffix of the function name
        // This handles cases like "foo" matching "crate::module::foo"
        let name_parts: Vec<&str> = base_name.split("::").collect();
        let path_parts: Vec<&str> = self.function_path.split("::").collect();

        if path_parts.len() <= name_parts.len() {
            let suffix = &name_parts[name_parts.len() - path_parts.len()..];
            return suffix == path_parts.as_slice();
        }

        false
    }

    /// Strip async-related suffixes from function names.
    ///
    /// Async functions in Rust are compiled into state machines, and their
    /// bodies appear with suffixes like `{closure#0}`, `{async_block#0}`, etc.
    fn strip_async_suffix(name: &str) -> &str {
        // Find the start of any `{...}` suffix
        if let Some(brace_pos) = name.find("::{") {
            &name[..brace_pos]
        } else {
            name
        }
    }
}

impl MirVisitor for FindVariablesByName<'_> {
    fn visit_func(&mut self, func: &Function) {
        self.current_function_name = func.name.clone();
    }

    fn visit_decl(&mut self, decl: &MirDecl) {
        if !self.matches_function(&self.current_function_name) {
            return;
        }

        if let MirDecl::User {
            local, name, span, ..
        } = decl
            && name == self.variable_name
        {
            self.found.push(VariableInfo {
                local: *local,
                name: name.clone(),
                span: *span,
                function_name: self.current_function_name.clone(),
            });
        }
    }
}

/// Type of decoration for a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DecoType {
    Lifetime,
    ImmBorrow,
    MutBorrow,
    Move,
    Call,
    SharedMut,
    Outlive,
}

impl DecoType {
    fn color(&self) -> &'static str {
        match self {
            DecoType::Lifetime => colors::GREEN,
            DecoType::ImmBorrow => colors::CYAN,
            DecoType::MutBorrow => colors::PURPLE,
            DecoType::Move => colors::YELLOW,
            DecoType::Call => colors::YELLOW,
            DecoType::SharedMut => colors::RED,
            DecoType::Outlive => colors::RED,
        }
    }
}

/// CLI renderer for decorations.
pub struct CliRenderer<'a> {
    source: &'a str,
    lines: Vec<&'a str>,
}

impl<'a> CliRenderer<'a> {
    pub fn new(source: &'a str) -> Self {
        let lines: Vec<&str> = source.lines().collect();
        Self { source, lines }
    }

    /// Render a single variable's decorations to the terminal.
    pub fn render_variable(
        &self,
        var_info: &VariableInfo,
        var_index: usize,
        total_vars: usize,
        decos: &[Deco],
    ) {
        // Print header
        println!(
            "\n{}=== Variable '{}' ({}/{}) in function '{}' ==={}\n",
            colors::CYAN,
            var_info.name,
            var_index + 1,
            total_vars,
            var_info.function_name,
            colors::RESET
        );

        // Group decorations by line
        let mut line_decos: HashMap<u32, Vec<(u32, u32, DecoType)>> = HashMap::new();

        for deco in decos {
            let (range, deco_type) = match deco {
                Deco::Lifetime { range, .. } => (*range, DecoType::Lifetime),
                Deco::ImmBorrow { range, .. } => (*range, DecoType::ImmBorrow),
                Deco::MutBorrow { range, .. } => (*range, DecoType::MutBorrow),
                Deco::Move { range, .. } => (*range, DecoType::Move),
                Deco::Call { range, .. } => (*range, DecoType::Call),
                Deco::SharedMut { range, .. } => (*range, DecoType::SharedMut),
                Deco::Outlive { range, .. } => (*range, DecoType::Outlive),
            };

            let (start_line, start_col) = utils::index_to_line_char(self.source, range.from());
            let (end_line, end_col) = utils::index_to_line_char(self.source, range.until());

            // Handle single-line decorations
            if start_line == end_line {
                line_decos
                    .entry(start_line)
                    .or_default()
                    .push((start_col, end_col, deco_type));
            } else {
                // Handle multi-line decorations by adding to each line
                for line in start_line..=end_line {
                    let col_start = if line == start_line { start_col } else { 0 };
                    let col_end = if line == end_line {
                        end_col
                    } else {
                        self.lines
                            .get(line as usize)
                            .map(|l| l.len() as u32)
                            .unwrap_or(0)
                    };
                    line_decos
                        .entry(line)
                        .or_default()
                        .push((col_start, col_end, deco_type));
                }
            }
        }

        // Find the range of lines to display
        let mut min_line = u32::MAX;
        let mut max_line = 0u32;

        for &line in line_decos.keys() {
            min_line = min_line.min(line);
            max_line = max_line.max(line);
        }

        // Add context lines (2 lines before and after)
        let context = 2;
        let display_start = min_line.saturating_sub(context);
        let display_end = (max_line + context).min(self.lines.len() as u32 - 1);

        // Print lines with decorations
        for line_num in display_start..=display_end {
            if let Some(line_content) = self.lines.get(line_num as usize) {
                // Print line number and syntax-highlighted content
                println!(
                    "{}{:4} |{} {}{}",
                    colors::DIM,
                    line_num + 1,
                    colors::RESET,
                    syntax::highlight(line_content),
                    colors::RESET
                );

                // Print decorations for this line
                if let Some(decos_for_line) = line_decos.get(&line_num) {
                    self.print_decorations(decos_for_line);
                }
            }
        }

        println!();
    }

    /// Print decoration underlines for a single line.
    /// Groups decorations of the same type on the same output line.
    fn print_decorations(&self, decos: &[(u32, u32, DecoType)]) {
        // Group decorations by type
        let mut by_type: HashMap<DecoType, Vec<(u32, u32)>> = HashMap::new();
        for (start, end, deco_type) in decos {
            by_type.entry(*deco_type).or_default().push((*start, *end));
        }

        // Sort types by the earliest start position for consistent output order
        let mut types_with_min_start: Vec<(DecoType, u32)> = by_type
            .iter()
            .map(|(deco_type, ranges)| {
                let min_start = ranges.iter().map(|(s, _)| *s).min().unwrap_or(0);
                (*deco_type, min_start)
            })
            .collect();
        types_with_min_start.sort_by_key(|(_, start)| *start);

        // Print each decoration type on its own line
        for (deco_type, _) in types_with_min_start {
            let ranges = &by_type[&deco_type];
            let mut sorted_ranges = ranges.clone();
            sorted_ranges.sort_by_key(|(start, _)| *start);

            // Build the underline string with all ranges of this type
            let max_end = sorted_ranges.iter().map(|(_, e)| *e).max().unwrap_or(0) as usize;
            let mut underline_chars = vec![' '; max_end + 1];

            for (start, end) in &sorted_ranges {
                for i in (*start as usize)..=(*end as usize).min(underline_chars.len() - 1) {
                    underline_chars[i] = '~';
                }
            }

            // Convert to string, trimming trailing spaces
            let underline: String = underline_chars.into_iter().collect();
            let underline = underline.trim_end();

            let prefix = format!("{}     |{} ", colors::DIM, colors::RESET);
            println!(
                "{}{}{}{}",
                prefix,
                deco_type.color(),
                underline,
                colors::RESET,
            );
        }
    }
}

/// Find a file in the crate data by path.
pub fn find_file<'a>(crate_data: &'a Crate, file_path: &Path) -> Option<&'a File> {
    let file_path_str = file_path.to_string_lossy();

    // Try exact match first
    if let Some(file) = crate_data.0.get(file_path_str.as_ref()) {
        return Some(file);
    }

    // Try matching by file name or relative path
    for (path, file) in &crate_data.0 {
        if path.ends_with(file_path_str.as_ref()) || file_path_str.ends_with(path) {
            return Some(file);
        }
    }

    None
}

/// Main entry point for CLI visualization with optional file path.
///
/// Shows ownership and lifetime visualization for a specific variable
/// in a function within the analyzed crate data.
pub fn show_variable(
    crate_data: &Crate,
    file_path: Option<&Path>,
    function_path: &str,
    variable_name: &str,
) -> Result<(), VisualizeError> {
    // Collect all matching variables across files
    let mut all_found: Vec<(String, VariableInfo)> = Vec::new();

    if let Some(path) = file_path {
        // Search in specific file
        let file = find_file(crate_data, path)
            .ok_or_else(|| VisualizeError::FileNotFound(path.display().to_string()))?;

        let mut finder = FindVariablesByName::new(function_path, variable_name);
        for func in &file.items {
            utils::mir_visit(func, &mut finder);
        }

        for var in finder.found {
            all_found.push((path.to_string_lossy().to_string(), var));
        }
    } else {
        // Search in all files
        for (file_path_str, file) in &crate_data.0 {
            let mut finder = FindVariablesByName::new(function_path, variable_name);
            for func in &file.items {
                utils::mir_visit(func, &mut finder);
            }

            for var in finder.found {
                all_found.push((file_path_str.clone(), var));
            }
        }
    }

    if all_found.is_empty() {
        return Err(VisualizeError::VariableNotFound(format!(
            "'{variable_name}' in function '{function_path}'"
        )));
    }

    let total_vars = all_found.len();

    // Display each found variable
    for (idx, (file_path_str, var_info)) in all_found.iter().enumerate() {
        let file_path = Path::new(file_path_str);

        // Get the file data for calculating decorations
        let file = crate_data
            .0
            .get(file_path_str)
            .ok_or_else(|| VisualizeError::FileNotFound(file_path_str.clone()))?;

        // Read the source file
        let source = std::fs::read_to_string(file_path)?;
        let renderer = CliRenderer::new(&source);

        // Calculate decorations for this variable
        let mut calc = CalcDecos::new(std::iter::once(var_info.local));
        for func in &file.items {
            utils::mir_visit(func, &mut calc);
        }
        calc.handle_overlapping();
        let decos = calc.decorations();

        renderer.render_variable(var_info, idx, total_vars, &decos);
    }

    // Print legend
    print_legend();

    Ok(())
}

/// Print a color legend for the different decoration types.
fn print_legend() {
    println!("{}Legend:{}", colors::CYAN, colors::RESET);
    println!("  {}~~~{} lifetime", colors::GREEN, colors::RESET);
    println!("  {}~~~{} immutable borrow", colors::CYAN, colors::RESET);
    println!("  {}~~~{} mutable borrow", colors::PURPLE, colors::RESET);
    println!("  {}~~~{} move / call", colors::YELLOW, colors::RESET);
    println!(
        "  {}~~~{} outlive / shared mutable",
        colors::RED,
        colors::RESET
    );
}
