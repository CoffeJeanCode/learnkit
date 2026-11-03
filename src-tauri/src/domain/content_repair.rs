//! Deterministic repair of model-generated textual content BEFORE pedagogical
//! review and grounding run.
//!
//! Inspired by OpenCode, Hermes and production LLM harnesses:
//! When a model generates options with bullet prefixes ("A)", "1."), surrounding
//! quotes, or leaks explanatory justification clauses ("... porque ..."),
//! deterministic code can resolve them in microseconds without spending full
//! round-trips or risking cascading rejections.

use crate::domain::notebook::{DiagnosticBattery, DiagnosticQuestion, GeneratedSectionBlock};

/// Justification connectors that should be detached from option text.
pub const EMBEDDED_JUSTIFICATION_CONNECTORS: [&str; 6] = [
    "porque",
    "ya que",
    "debido a",
    "puesto que",
    "lo cual significa",
    "es decir que",
];

/// Strips bullet prefixes (e.g. "A)", "1.", "- ") and wrapping quotation marks.
pub fn strip_option_prefixes(text: &str) -> String {
    let mut s = text.trim();

    // Strip outer quotes (double, single, guillemets) using char-safe stripping
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s = &s[1..s.len() - 1].trim();
    } else if s.starts_with('«') && s.ends_with('»') {
        if let Some(inner) = s.strip_prefix('«').and_then(|t| t.strip_suffix('»')) {
            s = inner.trim();
        }
    }

    // Strip common bullet markers:
    // "A) ", "a) ", "A. ", "a. ", "1) ", "1. ", "- ", "* "
    let bytes = s.as_bytes();
    if bytes.len() >= 3 {
        let first = bytes[0];
        let second = bytes[1];
        let third = bytes[2];
        if (first.is_ascii_alphabetic() || first.is_ascii_digit())
            && (second == b')' || second == b'.' || second == b'-' || second == b':')
            && third == b' '
        {
            s = s[3..].trim_start();
        } else if (first == b'-' || first == b'*') && second == b' ' {
            s = s[2..].trim_start();
        }
    }

    // Secondary check for "[A] " or "(A) "
    if s.len() >= 4 && (s.starts_with('[') || s.starts_with('(')) {
        if let Some(close_idx) = s.find(|c| c == ']' || c == ')') {
            if close_idx <= 3 && s.len() > close_idx + 1 && s.as_bytes()[close_idx + 1] == b' ' {
                s = s[close_idx + 2..].trim_start();
            }
        }
    }

    s.to_string()
}

/// Splits an option containing an embedded justification into (clean_option, justification_clause).
///
/// Example:
/// `"Incrementa la latencia porque duplica los viajes de red"` ->
/// `Some(("Incrementa la latencia", "porque duplica los viajes de red"))`
pub fn extract_embedded_justification(text: &str) -> Option<(String, String)> {
    let lower = text.to_lowercase();
    for connector in EMBEDDED_JUSTIFICATION_CONNECTORS {
        // Look for the connector preceded by whitespace, comma or dash
        if let Some(pos) = lower.find(connector) {
            if pos == 0 {
                continue;
            }
            let char_before = lower.as_bytes()[pos - 1];
            if !char_before.is_ascii_whitespace() && char_before != b',' && char_before != b'-' && char_before != b'(' {
                continue;
            }

            let left = text[..pos].trim_end_matches([' ', ',', ';', '-', '(', ':']).trim();
            let right = text[pos..].trim_start_matches([' ', ':']).trim_end_matches([' ', ')', '.']).trim();

            // Only split if the remaining left part is a meaningful option (>= 4 chars)
            if left.chars().count() >= 4 && !right.is_empty() {
                return Some((left.to_string(), right.to_string()));
            }
        }
    }
    None
}

/// Normalizes a single option: strips bullet prefixes, outer quotes, and detaches embedded justifications.
pub fn clean_option_text(text: &str) -> (String, Option<String>) {
    let stripped = strip_option_prefixes(text);
    if let Some((clean, just)) = extract_embedded_justification(&stripped) {
        (clean, Some(just))
    } else {
        (stripped, None)
    }
}

/// Repairs a `DiagnosticQuestion` in place:
/// 1. Strips prefixes and embedded justifications from all options.
/// 2. If justifications were detached and `diagnostic_insight` is short or default, enriches it.
/// 3. Synchronizes `correct_option` so it matches the corresponding cleaned option.
pub fn repair_diagnostic_question(q: &mut DiagnosticQuestion) -> usize {
    let mut fixes = 0;
    let old_correct = q.correct_option.clone();
    let old_correct_stripped = strip_option_prefixes(&old_correct);

    let mut detached_justifications = Vec::new();
    let mut matched_new_correct = None;

    for (i, opt) in q.options.iter_mut().enumerate() {
        let (cleaned, justification) = clean_option_text(opt);
        if cleaned != *opt {
            if let Some(just) = justification {
                detached_justifications.push(format!("{cleaned}: {just}"));
            }
            // Check if this option was the correct option
            if *opt == old_correct || strip_option_prefixes(opt) == old_correct_stripped || opt.starts_with(&old_correct) {
                matched_new_correct = Some(cleaned.clone());
            }
            *opt = cleaned;
            fixes += 1;
        } else if *opt == old_correct || strip_option_prefixes(opt) == old_correct_stripped {
            matched_new_correct = Some(opt.clone());
        }
        let _ = i;
    }

    // Update correct_option if we cleaned it or if stripping made it match
    if let Some(new_correct) = matched_new_correct {
        if q.correct_option != new_correct {
            q.correct_option = new_correct;
            fixes += 1;
        }
    } else {
        // Fallback: clean correct_option directly
        let (cleaned_correct, _) = clean_option_text(&q.correct_option);
        if cleaned_correct != q.correct_option {
            q.correct_option = cleaned_correct;
            fixes += 1;
        }
    }

    // Enrich diagnostic_insight if detached justifications exist and insight is brief
    if !detached_justifications.is_empty() && q.diagnostic_insight.trim().len() < 30 {
        let addition = detached_justifications.join(". ");
        if q.diagnostic_insight.trim().is_empty() {
            q.diagnostic_insight = addition;
        } else {
            q.diagnostic_insight = format!("{}. {}", q.diagnostic_insight.trim_end_matches('.'), addition);
        }
        fixes += 1;
    }

    fixes
}

/// Deterministically repairs all questions in a `DiagnosticBattery`.
pub fn repair_diagnostic_battery(battery: &mut DiagnosticBattery) -> usize {
    let mut fixes = 0;
    battery.goal_alignment = battery.goal_alignment.trim().to_string();
    for q in &mut battery.questions {
        fixes += repair_diagnostic_question(q);
    }
    fixes
}

/// Repairs block-level choices (e.g. `InteractivePredictionGate`) in place.
pub fn repair_block_content(block: &mut GeneratedSectionBlock) -> usize {
    match block {
        GeneratedSectionBlock::InteractivePredictionGate {
            options,
            correct_option,
            conceptual_feedback_map,
            ..
        } => {
            let mut fixes = 0;
            let old_correct = correct_option.clone();
            let mut matched_new_correct = None;

            for opt in options.iter_mut() {
                let (cleaned, just) = clean_option_text(opt);
                if cleaned != *opt {
                    let old_key = opt.clone();
                    if old_key == old_correct || strip_option_prefixes(&old_key) == strip_option_prefixes(&old_correct) {
                        matched_new_correct = Some(cleaned.clone());
                    }
                    if let Some(feedback) = conceptual_feedback_map.remove(&old_key) {
                        conceptual_feedback_map.insert(cleaned.clone(), feedback);
                    } else if let Some(justification) = just {
                        conceptual_feedback_map.entry(cleaned.clone()).or_insert(justification);
                    }
                    *opt = cleaned;
                    fixes += 1;
                } else if *opt == old_correct {
                    matched_new_correct = Some(opt.clone());
                }
            }

            if let Some(new_correct) = matched_new_correct {
                if *correct_option != new_correct {
                    *correct_option = new_correct;
                    fixes += 1;
                }
            } else {
                let (cleaned_correct, _) = clean_option_text(correct_option);
                if *correct_option != cleaned_correct {
                    *correct_option = cleaned_correct;
                    fixes += 1;
                }
            }
            fixes
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::DiagnosticDimension;

    #[test]
    fn test_strip_option_prefixes() {
        assert_eq!(strip_option_prefixes("A) Opción primera"), "Opción primera");
        assert_eq!(strip_option_prefixes("b. Segunda alternativa"), "Segunda alternativa");
        assert_eq!(strip_option_prefixes("1) Tercera opción"), "Tercera opción");
        assert_eq!(strip_option_prefixes("[C] Cuarta opción"), "Cuarta opción");
        assert_eq!(strip_option_prefixes("(D) Quinta opción"), "Quinta opción");
        assert_eq!(strip_option_prefixes("- Opción con guion"), "Opción con guion");
        assert_eq!(strip_option_prefixes("\"Texto entre comillas\""), "Texto entre comillas");
        assert_eq!(strip_option_prefixes("«Texto entre comillas latinas»"), "Texto entre comillas latinas");
    }

    #[test]
    fn test_extract_embedded_justification() {
        let opt = "Aumenta el throughput porque reduce los bloqueos";
        let res = extract_embedded_justification(opt);
        assert!(res.is_some());
        let (clean, just) = res.unwrap();
        assert_eq!(clean, "Aumenta el throughput");
        assert_eq!(just, "porque reduce los bloqueos");

        let opt2 = "Opción simple sin conectores";
        assert!(extract_embedded_justification(opt2).is_none());
    }

    #[test]
    fn test_repair_diagnostic_question() {
        let mut q = DiagnosticQuestion {
            dimension: DiagnosticDimension::Intuition,
            prompt: "¿Cuál es la causa?".to_string(),
            options: vec![
                "A) Duplica la memoria porque aloja dos buffers".to_string(),
                "B) Mantiene el tamaño constante".to_string(),
                "C) Libera el heap de inmediato".to_string(),
            ],
            correct_option: "A) Duplica la memoria porque aloja dos buffers".to_string(),
            diagnostic_insight: "".to_string(),
        };

        let fixes = repair_diagnostic_question(&mut q);
        assert!(fixes >= 2);
        assert_eq!(q.options[0], "Duplica la memoria");
        assert_eq!(q.correct_option, "Duplica la memoria");
        assert!(q.options.contains(&q.correct_option));
        assert!(q.diagnostic_insight.contains("porque aloja dos buffers"));
    }
}
