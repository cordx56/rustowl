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

/// ANSI color codes for different decoration types.
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED: &str = "\x1b[31m";
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
            VisualizeError::FileNotFound(path) => write!(f, "File not found: {}", path),
            VisualizeError::FunctionNotFound(name) => write!(f, "Function not found: {}", name),
            VisualizeError::VariableNotFound(name) => write!(f, "Variable not found: {}", name),
            VisualizeError::SourceReadError(e) => write!(f, "Failed to read source file: {}", e),
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
    function_name: &'a str,
    variable_name: &'a str,
    current_function_name: String,
    found: Vec<VariableInfo>,
}

impl<'a> FindVariablesByName<'a> {
    fn new(function_name: &'a str, variable_name: &'a str) -> Self {
        Self {
            function_name,
            variable_name,
            current_function_name: String::new(),
            found: Vec::new(),
        }
    }

    fn matches_function(&self, name: &str) -> bool {
        // Match the function name exactly or as the last part of a qualified path
        name == self.function_name || name.ends_with(&format!("::{}", self.function_name))
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
#[derive(Debug, Clone, Copy)]
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
            DecoType::ImmBorrow => colors::BLUE,
            DecoType::MutBorrow => colors::MAGENTA,
            DecoType::Move => colors::YELLOW,
            DecoType::Call => colors::YELLOW,
            DecoType::SharedMut => colors::RED,
            DecoType::Outlive => colors::RED,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            DecoType::Lifetime => "lifetime",
            DecoType::ImmBorrow => "immutable borrow",
            DecoType::MutBorrow => "mutable borrow",
            DecoType::Move => "move",
            DecoType::Call => "call",
            DecoType::SharedMut => "shared mutable",
            DecoType::Outlive => "outlive",
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
            colors::BLUE,
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
                    let col_start = if line == start_line {
                        start_col
                    } else {
                        0
                    };
                    let col_end = if line == end_line {
                        end_col
                    } else {
                        self.lines.get(line as usize).map(|l| l.len() as u32).unwrap_or(0)
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
                // Print line number and content
                println!("{:4}| {}", line_num + 1, line_content);

                // Print decorations for this line
                if let Some(decos_for_line) = line_decos.get(&line_num) {
                    self.print_decorations(decos_for_line);
                }
            }
        }

        println!();
    }

    /// Print decoration underlines for a single line.
    fn print_decorations(&self, decos: &[(u32, u32, DecoType)]) {
        // Sort decorations by start column
        let mut sorted_decos = decos.to_vec();
        sorted_decos.sort_by_key(|(start, _, _)| *start);

        // Group decorations by type for cleaner output
        let mut by_type: HashMap<&'static str, Vec<(u32, u32)>> = HashMap::new();
        for (start, end, deco_type) in &sorted_decos {
            by_type
                .entry(deco_type.label())
                .or_default()
                .push((*start, *end));
        }

        // Print each decoration type on its own line
        for (start, end, deco_type) in &sorted_decos {
            let prefix = "    | ";
            let spaces = " ".repeat(*start as usize);
            let underline_len = (*end - *start).max(1) as usize;
            let underline = "~".repeat(underline_len);

            println!(
                "{}{}{}{}{}  <- {}{}",
                prefix,
                spaces,
                deco_type.color(),
                underline,
                colors::RESET,
                deco_type.label(),
                colors::RESET
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

/// Main entry point for CLI visualization.
///
/// Shows ownership and lifetime visualization for a specific variable
/// in a function within the analyzed crate data.
pub fn show_variable(
    crate_data: &Crate,
    file_path: &Path,
    function_name: &str,
    variable_name: &str,
) -> Result<(), VisualizeError> {
    // Find the file in the crate data
    let file = find_file(crate_data, file_path)
        .ok_or_else(|| VisualizeError::FileNotFound(file_path.display().to_string()))?;

    // Find variables matching the name in the specified function
    let mut finder = FindVariablesByName::new(function_name, variable_name);
    for func in &file.items {
        utils::mir_visit(func, &mut finder);
    }

    if finder.found.is_empty() {
        return Err(VisualizeError::VariableNotFound(format!(
            "'{}' in function '{}'",
            variable_name, function_name
        )));
    }

    // Read the source file
    let source = std::fs::read_to_string(file_path)?;
    let renderer = CliRenderer::new(&source);

    // For each found variable, calculate and display decorations
    for (idx, var_info) in finder.found.iter().enumerate() {
        // Calculate decorations for this variable
        let mut calc = CalcDecos::new(std::iter::once(var_info.local));
        for func in &file.items {
            utils::mir_visit(func, &mut calc);
        }
        calc.handle_overlapping();
        let decos = calc.decorations();

        renderer.render_variable(var_info, idx, finder.found.len(), &decos);
    }

    // Print legend
    print_legend();

    Ok(())
}

/// Print a color legend for the different decoration types.
fn print_legend() {
    println!("{}Legend:{}", colors::BLUE, colors::RESET);
    println!(
        "  {}~~~{} lifetime",
        colors::GREEN,
        colors::RESET
    );
    println!(
        "  {}~~~{} immutable borrow",
        colors::BLUE,
        colors::RESET
    );
    println!(
        "  {}~~~{} mutable borrow",
        colors::MAGENTA,
        colors::RESET
    );
    println!(
        "  {}~~~{} move / call",
        colors::YELLOW,
        colors::RESET
    );
    println!(
        "  {}~~~{} outlive / shared mutable",
        colors::RED,
        colors::RESET
    );
}
