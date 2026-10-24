//! Best-effort parse of a JSON document that has only partly arrived — the
//! argument fragments of a streamed tool call. Used to show a block's text
//! while the model is still writing it.
//!
//! The strategy is "close what is open": append the missing `"`/`]`/`}` and
//! parse. When the cut falls somewhere a closer cannot fix (mid-key, right
//! after a `:`, inside `tru`, a lone `-`), fall back to the nearest earlier
//! point where a document is complete — before a top-level-of-its-container
//! comma, or just after an opening bracket. Every such cut point yields valid
//! JSON, so a result always exists once the document has opened.

use serde_json::Value;

/// Where a prefix may be cut so that closing the open containers yields valid
/// JSON, with the container stack that applies at that point.
struct CutPoint {
    at: usize,
    stack: Vec<char>,
}

/// Parses as much of `prefix` as it already says. `None` until the root
/// object/array has opened.
pub fn parse_partial(prefix: &str) -> Option<Value> {
    let trimmed = prefix.trim_start();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return None;
    }

    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut cuts: Vec<CutPoint> = Vec::new();
    for (i, c) in trimmed.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => {
                stack.push('}');
                cuts.push(CutPoint { at: i + 1, stack: stack.clone() });
            }
            '[' => {
                stack.push(']');
                cuts.push(CutPoint { at: i + 1, stack: stack.clone() });
            }
            '}' | ']' => {
                stack.pop();
            }
            ',' => cuts.push(CutPoint { at: i, stack: stack.clone() }),
            _ => {}
        }
    }

    // 1. The whole prefix, closing an open string and the open containers.
    //    This is the only candidate that shows a half-written string value.
    let mut whole = trimmed.to_string();
    if in_string {
        if escaped {
            whole.pop(); // a lone trailing backslash
        }
        drop_incomplete_unicode_escape(&mut whole);
        whole.push('"');
    }
    whole.extend(stack.iter().rev());
    if let Ok(v) = serde_json::from_str::<Value>(&whole) {
        return Some(v);
    }

    // 2. Nearest earlier cut points, latest first. A handful is enough: only
    //    the tail of the document can be in an unfixable state.
    for cut in cuts.iter().rev().take(8) {
        let mut candidate = trimmed[..cut.at].to_string();
        candidate.extend(cut.stack.iter().rev());
        if let Ok(v) = serde_json::from_str::<Value>(&candidate) {
            return Some(v);
        }
    }
    None
}

/// Removes a trailing, not-yet-complete `\uXXXX` escape (inside an open
/// string) that would make the closed document invalid.
fn drop_incomplete_unicode_escape(s: &mut String) {
    let bytes = s.as_bytes();
    // Look back at most 5 bytes for a `\u` followed by fewer than 4 hex digits.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nothing_until_the_root_opens() {
        assert_eq!(parse_partial(""), None);
        assert_eq!(parse_partial("   "), None);
        assert_eq!(parse_partial("garbage"), None);
    }

    #[test]
    fn a_complete_document_parses_unchanged() {
        assert_eq!(parse_partial(r#"{"a":1,"b":["x","y"]}"#), Some(json!({"a": 1, "b": ["x", "y"]})));
    }

    #[test]
    fn a_half_written_string_value_is_shown() {
        assert_eq!(parse_partial(r#"{"title":"Recurs"#), Some(json!({"title": "Recurs"})));
        assert_eq!(parse_partial(r#"{"blockType":"anchored_micro_theory","title":"Hola mu"#), Some(json!({"blockType": "anchored_micro_theory", "title": "Hola mu"})));
    }

    #[test]
    fn a_cut_inside_a_key_falls_back_to_the_last_complete_field() {
        assert_eq!(parse_partial(r#"{"title":"A","sys"#), Some(json!({"title": "A"})));
        assert_eq!(parse_partial(r#"{"title":"A","systemRule":"#), Some(json!({"title": "A"})));
        assert_eq!(parse_partial(r#"{"title":"A","systemRule""#), Some(json!({"title": "A"})));
    }

    #[test]
    fn a_cut_inside_a_literal_or_number_falls_back() {
        assert_eq!(parse_partial(r#"{"a":1,"b":tr"#), Some(json!({"a": 1})));
        assert_eq!(parse_partial(r#"{"a":1,"b":-"#), Some(json!({"a": 1})));
        assert_eq!(parse_partial(r#"{"a":1,"b":12"#), Some(json!({"a": 1, "b": 12})));
    }

    #[test]
    fn nested_containers_are_closed() {
        assert_eq!(parse_partial(r#"{"steps":[{"t":"uno"},{"t":"do"#), Some(json!({"steps": [{"t": "uno"}, {"t": "do"}]})));
        assert_eq!(parse_partial(r#"{"steps":["a","#), Some(json!({"steps": ["a"]})));
    }

    #[test]
    fn an_open_root_with_nothing_yet_is_an_empty_object() {
        assert_eq!(parse_partial("{"), Some(json!({})));
        assert_eq!(parse_partial(r#"{""#), Some(json!({})));
    }

    #[test]
    fn escapes_cut_in_half_do_not_break_the_parse() {
        // A lone trailing backslash.
        assert_eq!(parse_partial("{\"t\":\"a\\"), Some(json!({"t": "a"})));
        // A \u escape missing digits.
        assert_eq!(parse_partial("{\"t\":\"a\\u00"), Some(json!({"t": "a"})));
        // A complete escape stays.
        assert_eq!(parse_partial("{\"t\":\"a\\n"), Some(json!({"t": "a\n"})));
        assert_eq!(parse_partial("{\"t\":\"a\\u00e9"), Some(json!({"t": "aé"})));
    }

    #[test]
    fn braces_and_commas_inside_strings_are_not_structure() {
        assert_eq!(parse_partial(r#"{"t":"a,{b]","u":"c"#), Some(json!({"t": "a,{b]", "u": "c"})));
    }

    #[test]
    fn every_prefix_of_a_real_document_either_parses_or_is_not_yet_open() {
        let doc = r#"{"blockType":"anchored_micro_theory","title":"Recursión","steps":[{"n":1,"t":"caso base"},{"n":2,"t":"paso \"recursivo\" é"}],"ok":true,"x":-12.5e1}"#;
        for end in 0..=doc.len() {
            if !doc.is_char_boundary(end) {
                continue;
            }
            let got = parse_partial(&doc[..end]);
            if end > 0 {
                assert!(got.is_some(), "prefix of {end} bytes must parse: {:?}", &doc[..end]);
            }
        }
        assert_eq!(parse_partial(doc), serde_json::from_str(doc).ok());
    }
}
