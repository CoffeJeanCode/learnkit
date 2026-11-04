//! Fault-tolerant JSON parser & auto-repair engine (inspired by Python `json_repair` and JS `jsonrepair`).
//!
//! Automatically repairs:
//! 1. Incomplete/truncated JSON (EOF inside strings, arrays, or objects) by closing open strings and containers in reverse order.
//! 2. Trailing commas before `}` or `]` or at the end of input.
//! 3. Single-quoted strings (`'foo'`) converted to standard JSON double-quoted strings (`"foo"`), escaping inner double quotes.
//! 4. Unquoted object keys (`{ key: "value" }`) converted to quoted keys (`{ "key": "value" }`).
//! 5. Python literals (`True`, `False`, `None`) outside strings converted to `true`, `false`, `null`.
//! 6. Raw unescaped control characters (`\n`, `\r`, `\t`) inside strings converted to valid escape sequences.
//! 7. Incomplete key-value pairs (e.g. `{"key": `) completed with empty strings or null.
//! 8. Markdown code block fences (```` ```json ... ````) and XML/tool-call wrappers (`<tool_call> ... </tool_call>`) extracted.
//! 9. Deep fallback to [`crate::tools::partial_json::parse_partial`] if lexical repair alone does not yield a parseable document.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Extracts JSON candidate by stripping markdown fences, tool wrappers, or leading non-JSON prose.
pub fn extract_json_candidate(input: &str) -> &str {
    let mut s = input.trim();
    if let Some(start) = s.find("<tool_call>") {
        let after = &s[start + "<tool_call>".len()..];
        if let Some(end) = after.find("</tool_call>") {
            s = after[..end].trim();
        } else {
            s = after.trim();
        }
    }
    if let Some(start) = s.find("```") {
        let after = &s[start + 3..];
        let content_start = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = content_start.rfind("```") {
            s = content_start[..end].trim();
        } else {
            s = content_start.trim();
        }
    }
    if let Some(first_brace) = s.find('{') {
        let first_bracket = s.find('[').unwrap_or(usize::MAX);
        let root = first_brace.min(first_bracket);
        s = &s[root..];
    } else if let Some(first_bracket) = s.find('[') {
        s = &s[first_bracket..];
    }
    s
}

/// Repairs a malformed or truncated JSON string into valid JSON.
pub fn repair_json(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "{}".to_string();
    }

    // Fast path: if already valid JSON, keep as is
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return trimmed.to_string();
    }

    let candidate = extract_json_candidate(trimmed);

    // Lexical repair pass
    let lexically_repaired = lex_repair(candidate);
    if serde_json::from_str::<Value>(&lexically_repaired).is_ok() {
        return lexically_repaired;
    }

    // Fallback 1: Try parse_partial on lexically repaired output
    if let Some(val) = crate::tools::partial_json::parse_partial(&lexically_repaired) {
        if let Ok(s) = serde_json::to_string(&val) {
            return s;
        }
    }

    // Fallback 2: Try parse_partial directly on candidate
    if let Some(val) = crate::tools::partial_json::parse_partial(candidate) {
        if let Ok(s) = serde_json::to_string(&val) {
            return s;
        }
    }

    // Fallback 3: Try parse_partial on original trimmed string
    if let Some(val) = crate::tools::partial_json::parse_partial(trimmed) {
        if let Ok(s) = serde_json::to_string(&val) {
            return s;
        }
    }

    lexically_repaired
}

/// Lexical repair pass for unquoted keys, single quotes, unescaped control chars, trailing commas, and unclosed containers.
fn lex_repair(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    let mut in_string = false;
    let mut string_delim = '"';
    let mut escaped = false;
    let mut container_stack: Vec<char> = Vec::new();

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if in_string {
            if escaped {
                escaped = false;
                if string_delim == '\'' {
                    if ch == '\'' {
                        // \' in single-quoted string becomes ' in double-quoted string
                        out.push('\'');
                    } else if ch == '"' {
                        // \" in single-quoted string stays \"
                        out.push('\\');
                        out.push('"');
                    } else {
                        out.push('\\');
                        out.push(ch);
                    }
                } else {
                    out.push('\\');
                    out.push(ch);
                }
                i += 1;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                i += 1;
                continue;
            }

            if ch == string_delim {
                out.push('"');
                in_string = false;
                i += 1;
                continue;
            }

            if string_delim == '\'' && ch == '"' {
                out.push('\\');
                out.push('"');
                i += 1;
                continue;
            }

            match ch {
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                other => out.push(other),
            }
            i += 1;
            continue;
        }

        // --- Outside string ---
        if ch == '"' {
            in_string = true;
            string_delim = '"';
            out.push('"');
            i += 1;
            continue;
        }

        if ch == '\'' {
            in_string = true;
            string_delim = '\'';
            out.push('"');
            i += 1;
            continue;
        }

        if ch == '{' {
            container_stack.push('}');
            out.push('{');
            i += 1;
            continue;
        }

        if ch == '[' {
            container_stack.push(']');
            out.push('[');
            i += 1;
            continue;
        }

        if ch == '}' || ch == ']' {
            strip_trailing_comma(&mut out);
            if let Some(pos) = container_stack.iter().rposition(|&c| c == ch) {
                container_stack.truncate(pos);
            }
            out.push(ch);
            i += 1;
            if container_stack.is_empty() {
                // If closed root container, inspect if remaining text is just comments/prose
                let mut rest_idx = i;
                while rest_idx < len && chars[rest_idx].is_whitespace() {
                    rest_idx += 1;
                }
                if rest_idx < len && !chars[rest_idx..].iter().any(|&c| c == '{' || c == '[') {
                    break;
                }
            }
            continue;
        }

        if ch == ',' {
            strip_trailing_comma(&mut out);
            let trimmed = out.trim_end();
            if !trimmed.ends_with('{') && !trimmed.ends_with('[') {
                out.push(',');
            }
            i += 1;
            continue;
        }

        if ch == ':' {
            out.push(':');
            i += 1;
            continue;
        }

        if ch.is_whitespace() {
            out.push(ch);
            i += 1;
            continue;
        }

        // Unquoted key or keyword / identifier
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$' || chars[i] == '-') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            let mut peek = i;
            while peek < len && chars[peek].is_whitespace() {
                peek += 1;
            }
            let is_key = peek < len && chars[peek] == ':';

            if is_key {
                out.push('"');
                out.push_str(&word);
                out.push('"');
            } else {
                match word.as_str() {
                    "True" => out.push_str("true"),
                    "False" => out.push_str("false"),
                    "None" => out.push_str("null"),
                    "undefined" | "NaN" | "Infinity" => out.push_str("null"),
                    _ => out.push_str(&word),
                }
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    // --- EOF clean-up ---
    if in_string {
        if escaped {
            out.pop();
        }
        drop_incomplete_unicode_escape(&mut out);
        out.push('"');
    }

    strip_trailing_comma(&mut out);

    let trimmed = out.trim_end();
    if trimmed.ends_with(':') {
        out.push_str("\"\"");
    }

    strip_trailing_comma(&mut out);

    while let Some(closing) = container_stack.pop() {
        strip_trailing_comma(&mut out);
        out.push(closing);
    }

    out
}

fn strip_trailing_comma(out: &mut String) {
    let mut end = out.len();
    while end > 0 && out[..end].chars().last().is_some_and(|c| c.is_whitespace()) {
        end -= out[..end].chars().last().unwrap().len_utf8();
    }
    if end > 0 && out[..end].ends_with(',') {
        end -= 1;
        out.truncate(end);
    }
}

fn drop_incomplete_unicode_escape(s: &mut String) {
    let bytes = s.as_bytes();
    for back in 1..=5usize {
        if bytes.len() < back + 1 {
            break;
        }
        let start = bytes.len() - back - 1;
        if bytes[start] == b'\\' && bytes[start + 1] == b'u' {
            let hex = &bytes[start + 2..];
            if hex.len() < 4 && hex.iter().all(|b| b.is_ascii_hexdigit()) {
                s.truncate(start);
            }
            return;
        }
    }
}

/// Parses a string into `serde_json::Value`, automatically repairing any malformed/truncated syntax.
pub fn parse_repaired_value(input: &str) -> serde_json::Result<Value> {
    let trimmed = input.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Ok(v);
    }
    let repaired = repair_json(trimmed);
    if let Ok(v) = serde_json::from_str::<Value>(&repaired) {
        return Ok(v);
    }
    if let Some(v) = crate::tools::partial_json::parse_partial(&repaired) {
        return Ok(v);
    }
    if let Some(v) = crate::tools::partial_json::parse_partial(trimmed) {
        return Ok(v);
    }
    serde_json::from_str::<Value>(&repaired)
}

/// Deserializes a target type `T` from a JSON string, automatically repairing any malformed/truncated syntax.
pub fn parse_repaired_json<T: DeserializeOwned>(input: &str) -> serde_json::Result<T> {
    let trimmed = input.trim();
    if let Ok(v) = serde_json::from_str::<T>(trimmed) {
        return Ok(v);
    }
    let val = parse_repaired_value(trimmed)?;
    serde_json::from_value(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn repairs_eof_unclosed_string_and_object() {
        let input = r#"{"blockType": "anchored_micro_theory", "title": "Sistemas de Diseño", "intuitiveHook": "Imagina una caja"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["blockType"], "anchored_micro_theory");
        assert_eq!(val["title"], "Sistemas de Diseño");
        assert_eq!(val["intuitiveHook"], "Imagina una caja");
    }

    #[test]
    fn repairs_eof_unclosed_arrays_and_objects_deeply() {
        let input = r#"{"blockType": "interactive_prediction_gate", "options": ["A", "B", {"nested": "value"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["blockType"], "interactive_prediction_gate");
        assert_eq!(val["options"][0], "A");
        assert_eq!(val["options"][1], "B");
        assert_eq!(val["options"][2]["nested"], "value");
    }

    #[test]
    fn removes_trailing_commas_in_objects_and_arrays() {
        let input = r#"{"a": 1, "b": [10, 20, ], "c": {"d": "ok", }, }"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val, json!({"a": 1, "b": [10, 20], "c": {"d": "ok"}}));
    }

    #[test]
    fn normalizes_single_quoted_strings() {
        let input = r#"{'blockType': 'anchored_micro_theory', 'title': 'Don\'t worry "quotes"'}"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["blockType"], "anchored_micro_theory");
        assert_eq!(val["title"], "Don't worry \"quotes\"");
    }

    #[test]
    fn quotes_unquoted_keys() {
        let input = r#"{blockType: "anchored_micro_theory", title: "Tokens", count: 42}"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["blockType"], "anchored_micro_theory");
        assert_eq!(val["title"], "Tokens");
        assert_eq!(val["count"], 42);
    }

    #[test]
    fn handles_python_literals() {
        let input = r#"{"active": True, "failed": False, "details": None}"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["active"], true);
        assert_eq!(val["failed"], false);
        assert!(val["details"].is_null());
    }

    #[test]
    fn escapes_raw_newlines_inside_strings() {
        let input = "{\"code\": \"function hello() {\n  return 1;\n}\"}";
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["code"], "function hello() {\n  return 1;\n}");
    }

    #[test]
    fn handles_cut_after_colon() {
        let input = r#"{"blockType": "theory", "title": "#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["blockType"], "theory");
        assert_eq!(val["title"], "");
    }

    #[test]
    fn strips_markdown_fences_and_tool_call_tags() {
        let input = "```json\n{\"blockType\": \"tpl_theory\", \"title\": \"Fenced\"}\n```";
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["title"], "Fenced");

        let xml = "<tool_call><function=publish_notebook_block><arguments>{\"blockType\": \"tpl_theory\", \"title\": \"Tagged\"}</arguments></function></tool_call>";
        let val2 = parse_repaired_value(xml).expect("must parse xml wrapped json");
        assert_eq!(val2["title"], "Tagged");
    }

    #[test]
    fn ignores_trailing_prose_after_closed_object() {
        let input = r#"{"blockType": "anchored_micro_theory", "title": "Done"} ¡Espero que te guste este bloque!"#;
        let repaired = repair_json(input);
        let val: Value = serde_json::from_str(&repaired).expect("must parse");
        assert_eq!(val["title"], "Done");
    }
}
