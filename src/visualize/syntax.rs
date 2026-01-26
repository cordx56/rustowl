//! Simple Rust syntax highlighter using a state-machine based lexer.

use super::colors;

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield",
];

const PRIMITIVE_TYPES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64",
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
            && (chars[i + 1] == '[' || (chars[i + 1] == '!' && i + 2 < len && chars[i + 2] == '['))
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

            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
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
