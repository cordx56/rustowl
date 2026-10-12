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

mod syntax;

/// Styles for different decoration types and syntax highlighting.
pub(crate) mod colors {
    use anstyle::{AnsiColor, Color, Effects, Style};

    pub const GREEN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightGreen)));
    pub const CYAN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightCyan)));
    pub const PURPLE: Style =
        Style::new().fg_color(Some(Color::Ansi256(anstyle::Ansi256Color(177))));
    pub const MAGENTA: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightMagenta)));
    pub const YELLOW: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightYellow)));
    pub const RED: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightRed)));

    pub const DIM: Style = Style::new().effects(Effects::DIMMED);

    // Syntax highlighting colors
    pub const KEYWORD: Style = MAGENTA;
    pub const TYPE: Style = YELLOW;
    pub const STRING: Style = GREEN;
    pub const NUMBER: Style = CYAN;
    pub const COMMENT: Style = DIM;
    pub const LIFETIME: Style = YELLOW;
    pub const MACRO: Style = CYAN;
    pub const ATTRIBUTE: Style = MAGENTA;
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
    /// - Trait implementations: `Type::method` matches `<module::Type as Trait>::method`
    fn matches_function(&self, name: &str) -> bool {
        // For async functions, we need to match both the outer function and the closure
        // The actual code is in the closure, but we strip the suffix when matching
        let base_name = Self::strip_async_suffix(name);

        // For trait implementations, normalize the name to `Type::method` format
        // e.g., `<lsp::backend::Backend as tower_lsp::LanguageServer>::did_open`
        //    -> `lsp::backend::Backend::did_open`
        if let Some(normalized) = Self::normalize_trait_impl_name(base_name)
            && self.matches_normalized(&normalized)
        {
            return true;
        }

        self.matches_normalized(base_name)
    }

    /// Check if the normalized function name matches the search path.
    fn matches_normalized(&self, name: &str) -> bool {
        // Exact match
        if name == self.function_path {
            return true;
        }

        // Check if the function name ends with the given path
        // e.g., "module::foo" matches "crate::module::foo"
        if name.ends_with(&format!("::{}", self.function_path)) {
            return true;
        }

        // Check if the given path is a suffix of the function name
        // This handles cases like "foo" matching "crate::module::foo"
        let name_parts: Vec<&str> = name.split("::").collect();
        let path_parts: Vec<&str> = self.function_path.split("::").collect();

        if path_parts.len() <= name_parts.len() {
            let suffix = &name_parts[name_parts.len() - path_parts.len()..];
            return suffix == path_parts.as_slice();
        }

        false
    }

    /// Normalize trait implementation names to `Type::method` format.
    ///
    /// Converts `<module::Type as Trait>::method` to `module::Type::method`.
    /// Returns `None` if the name is not a trait implementation.
    fn normalize_trait_impl_name(name: &str) -> Option<String> {
        if !name.starts_with('<') {
            return None;
        }

        let as_pos = name.find(" as ")?;
        let gt_pos = name[as_pos..].find(">::")?;

        // Extract the type name (between '<' and ' as ')
        let type_name = &name[1..as_pos];
        // Extract the method name (after '>::')
        let method_start = as_pos + gt_pos + 3;
        let method_name = &name[method_start..];

        Some(format!("{type_name}::{method_name}"))
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

impl Deco {
    const COLOR_LIFETIME: anstyle::Style = colors::GREEN;
    const COLOR_IMMUTABLE: anstyle::Style = colors::CYAN;
    const COLOR_MUTABLE: anstyle::Style = colors::PURPLE;
    const COLOR_MOVE: anstyle::Style = colors::YELLOW;
    const COLOR_CALL: anstyle::Style = colors::YELLOW;
    const COLOR_SHARED: anstyle::Style = colors::RED;
    const COLOR_OUTLIVE: anstyle::Style = colors::RED;

    fn color(&self) -> anstyle::Style {
        match self {
            Deco::Lifetime { .. } => Self::COLOR_LIFETIME,
            Deco::DefinitelyLive { .. } => Self::COLOR_LIFETIME,
            Deco::MaybeInitialized { .. } => Self::COLOR_LIFETIME,
            Deco::ImmBorrow { .. } => Self::COLOR_IMMUTABLE,
            Deco::MutBorrow { .. } => Self::COLOR_MUTABLE,
            Deco::Move { .. } => Self::COLOR_MOVE,
            Deco::Call { .. } => Self::COLOR_CALL,
            Deco::SharedMut { .. } => Self::COLOR_SHARED,
            Deco::Outlive { .. } => Self::COLOR_OUTLIVE,
        }
    }

    const LINE_SOLID: char = '-';
    const LINE_WAVY: char = '~';

    fn underline_char(&self) -> char {
        match self {
            Deco::Lifetime { .. } => Self::LINE_WAVY,
            Deco::DefinitelyLive { .. } => Self::LINE_SOLID,
            Deco::MaybeInitialized { .. } => Self::LINE_WAVY,
            Deco::ImmBorrow { .. } => Self::LINE_SOLID,
            Deco::MutBorrow { .. } => Self::LINE_SOLID,
            Deco::Move { .. } => Self::LINE_SOLID,
            Deco::Call { .. } => Self::LINE_SOLID,
            Deco::SharedMut { .. } => Self::LINE_WAVY,
            Deco::Outlive { .. } => Self::LINE_WAVY,
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
        let style = colors::CYAN;
        println!(
            "\n{style}=== Variable '{}' ({}/{}) in function '{}' ==={style:#}\n",
            var_info.name,
            var_index + 1,
            total_vars,
            var_info.function_name,
        );

        // Group decorations by line
        let mut line_decos: HashMap<u32, Vec<(u32, u32, Deco)>> = HashMap::new();

        for deco in decos {
            let (range, overlapped) = match deco {
                Deco::Lifetime { .. } => {
                    // normal lifetime is not used now
                    continue;
                }
                Deco::DefinitelyLive {
                    range, overlapped, ..
                }
                | Deco::MaybeInitialized {
                    range, overlapped, ..
                }
                | Deco::ImmBorrow {
                    range, overlapped, ..
                }
                | Deco::MutBorrow {
                    range, overlapped, ..
                }
                | Deco::Move {
                    range, overlapped, ..
                }
                | Deco::Call {
                    range, overlapped, ..
                }
                | Deco::SharedMut {
                    range, overlapped, ..
                }
                | Deco::Outlive {
                    range, overlapped, ..
                } => (*range, *overlapped),
            };
            if overlapped {
                continue;
            }

            let (start_line, start_col) = utils::index_to_line_char(self.source, range.from());
            let (end_line, end_col) = utils::index_to_line_char(self.source, range.until());

            // Handle single-line decorations
            if start_line == end_line {
                line_decos
                    .entry(start_line)
                    .or_default()
                    .push((start_col, end_col, deco.clone()));
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
                        .push((col_start, col_end, deco.clone()));
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
                let dim = colors::DIM;
                println!(
                    "{dim}{:4} |{dim:#} {}",
                    line_num + 1,
                    syntax::highlight(line_content),
                );

                // Print decorations for this line
                if let Some(decos_for_line) = line_decos.get_mut(&line_num) {
                    self.print_decorations(decos_for_line);
                }
            }
        }

        println!();
    }

    /// Print decoration underlines for a single line.
    /// Groups decorations of the same type on the same output line.
    fn print_decorations(&self, decos: &mut [(u32, u32, Deco)]) {
        decos.sort_by_key(|(start, _, _)| *start);

        // print header
        let dim = colors::DIM;
        print!("{dim}     |{dim:#} ");

        let mut printed = 0;
        for (start, end, deco) in decos {
            if printed < *start {
                let space: String = (printed..*start).map(|_| ' ').collect();
                print!("{space}");
            }

            let color = deco.color();
            let line: String = (*start..*end).map(|_| deco.underline_char()).collect();
            print!("{color}{line}{color:#}");

            printed = *end;
        }
        println!();
    }
}

/// Find a file in the crate data by path.
pub fn find_file<'a>(crate_data: &'a Crate, file_path: &Path) -> Option<&'a File> {
    let file_path_str = normalize_file_path(file_path);

    // Try exact match first
    if let Some(file) = crate_data.0.get::<str>(&file_path_str) {
        return Some(file);
    }

    // Try matching by file name or relative path
    for (path, file) in &crate_data.0 {
        let normalized_path = normalize_file_path(Path::new(path));
        if normalized_path.ends_with(&file_path_str) || file_path_str.ends_with(&normalized_path) {
            return Some(file);
        }
    }

    None
}

fn normalize_file_path(path: &Path) -> String {
    #[cfg(not(windows))]
    let normalized = path.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    let mut normalized = path.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    {
        if let Some(stripped) = normalized.strip_prefix("//?/") {
            normalized = stripped.to_owned();
        }
        if let Some(stripped) = normalized.strip_prefix("//./") {
            normalized = stripped.to_owned();
        }
        normalized.make_ascii_lowercase();
    }

    normalized
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
        // Use find_file to normalize the path (handles UNC vs non-UNC paths on Windows)
        let file = find_file(crate_data, file_path)
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
    let cyan = Deco::COLOR_IMMUTABLE;
    let lifetime = Deco::COLOR_LIFETIME;
    let immutable = Deco::COLOR_IMMUTABLE;
    let mutable = Deco::COLOR_MUTABLE;
    let mov = Deco::COLOR_MOVE;
    let red = Deco::COLOR_OUTLIVE;

    println!("{cyan}Legend:{cyan:#}");
    println!(
        "  {lifetime}{}{}{}{lifetime:#} definitely live (lifetime)",
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
    );
    println!(
        "  {lifetime}{}{}{}{lifetime:#} maybe live",
        Deco::LINE_WAVY,
        Deco::LINE_WAVY,
        Deco::LINE_WAVY,
    );
    println!(
        "  {immutable}{}{}{}{immutable:#} immutable borrow",
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
    );
    println!(
        "  {mutable}{}{}{}{mutable:#} mutable borrow",
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
    );
    println!(
        "  {mov}{}{}{}{mov:#} move / call",
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
        Deco::LINE_SOLID,
    );
    println!(
        "  {red}{}{}{}{red:#} outlive / shared mutable",
        Deco::LINE_WAVY,
        Deco::LINE_WAVY,
        Deco::LINE_WAVY,
    );
}

#[cfg(test)]
mod tests {
    use super::find_file;
    use crate::lsp::decoration::{CalcDecos, Deco};
    use crate::models::*;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn find_file_matches_relative_suffix() {
        let crate_data = Crate(HashMap::from([(
            String::from("algo-tests/src/vec.rs"),
            File { items: vec![] },
        )]));

        let file = find_file(&crate_data, Path::new("./algo-tests/src/vec.rs"));
        assert!(file.is_some());
    }

    #[cfg(windows)]
    #[test]
    fn find_file_matches_windows_verbatim_path() {
        let crate_data = Crate(HashMap::from([(
            String::from("C:/repo/algo-tests/src/vec.rs"),
            File { items: vec![] },
        )]));

        let file = find_file(&crate_data, Path::new(r"\\?\C:\repo\algo-tests\src\vec.rs"));
        assert!(file.is_some());
    }

    fn range(from: u32, until: u32) -> Range {
        Range::new(Loc(from), Loc(until)).unwrap()
    }

    fn lifetime(local: u32, from: u32, until: u32) -> Deco {
        Deco::Lifetime {
            local: FnLocal::new(local, 1),
            range: range(from, until),
            hover_text: "lifetime".to_string(),
            overlapped: false,
        }
    }

    fn imm_borrow(local: u32, from: u32, until: u32) -> Deco {
        Deco::ImmBorrow {
            local: FnLocal::new(local, 1),
            range: range(from, until),
            hover_text: "borrow".to_string(),
            overlapped: false,
        }
    }

    /// Which decorations survive `handle_overlapping`, and in what order.
    fn survivors(decos: Vec<Deco>) -> Vec<(u8, Range)> {
        let mut calc = CalcDecos::with_decorations(decos);
        calc.handle_overlapping();
        calc.take_decorations()
            .iter()
            .map(|d| (CalcDecos::draw_order(d), CalcDecos::range_of(d)))
            .collect()
    }

    /// Disjoint ranges are untouched, but come back in draw order.
    #[test]
    fn cursor_disjoint_ranges_keep_draw_order() {
        assert_eq!(
            survivors(vec![imm_borrow(1, 10, 20), lifetime(1, 30, 40)]),
            vec![(0, range(30, 40)), (3, range(10, 20))],
            "disjoint ranges survive unchanged, in draw order"
        );
    }

    /// A range wholly inside another is cut out of the outer one.
    #[test]
    fn cursor_inner_range_is_excluded_from_outer() {
        assert_eq!(
            survivors(vec![lifetime(1, 0, 100), imm_borrow(2, 40, 50)]),
            vec![
                // the outer range is cut into three fragments, the first
                // being the span the inner decoration occupies...
                (0, range(40, 50)),
                (0, range(0, 39)),
                (0, range(51, 100)),
                // ...and the inner decoration is drawn on top of it
                (3, range(40, 50)),
            ],
        );
    }

    /// Exact duplicates collapse to one.
    #[test]
    fn cursor_identical_ranges_deduplicate() {
        let out = survivors(vec![lifetime(1, 5, 9), lifetime(1, 5, 9)]);
        assert_eq!(out.len(), 1);
    }

    fn call_deco(local: u32, from: u32, until: u32) -> Deco {
        Deco::Call {
            local: FnLocal::new(local, 1),
            range: range(from, until),
            hover_text: "function call".to_string(),
            overlapped: false,
        }
    }

    /// When the new span encloses an existing call, nothing changes: the
    /// inner call is already covered, so no outer call is added.
    #[test]
    fn cursor_enclosing_existing_call_is_a_no_op() {
        let mut calc = CalcDecos::with_decorations(vec![call_deco(1, 5, 10)]);
        // 0..100 encloses the existing 5..10
        calc.calc_call(
            &MirPlace {
                local: FnLocal::new(2, 1),
                projection: vec![],
            },
            range(0, 100),
        );
        let out: Vec<_> = calc
            .take_decorations()
            .iter()
            .map(CalcDecos::range_of)
            .collect();
        assert_eq!(
            out,
            vec![range(5, 10)],
            "an existing call inside the new span must suppress it"
        );
    }

    /// A new span contained by an existing call replaces it, so the inner
    /// decoration does not linger under the outer one.
    #[test]
    fn cursor_new_call_replaces_the_one_it_contains() {
        let mut calc = CalcDecos::with_decorations(vec![call_deco(1, 5, 10)]);
        calc.calc_call(
            &MirPlace {
                local: FnLocal::new(2, 1),
                projection: vec![],
            },
            range(6, 9),
        );
        let out: Vec<_> = calc
            .take_decorations()
            .iter()
            .map(CalcDecos::range_of)
            .collect();
        assert_eq!(
            out,
            vec![range(6, 9)],
            "the enclosing call must be replaced by the new one"
        );
    }

    #[test]
    fn cursor_empty_declaration_set_is_harmless() {
        let out = survivors(vec![]);
        assert!(out.is_empty());
    }
}
