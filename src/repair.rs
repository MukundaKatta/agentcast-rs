//! Three-pass local repair for LLM JSON output.
//!
//! 1. Strip markdown code fences (```json, ```).
//! 2. Extract the largest balanced JSON object from surrounding prose.
//! 3. Remove trailing commas inside arrays/objects.
//!
//! Returns the repaired text. Caller still has to parse — repair just
//! cleans up well-known shapes the model emits.

/// Apply all three repair passes in order.
pub fn repair(input: &str) -> String {
    let s = strip_fences(input);
    let s = extract_balanced(&s);
    strip_trailing_commas(&s)
}

fn strip_fences(s: &str) -> String {
    // Match ```json ... ``` or ``` ... ``` greedily across lines.
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Skip optional language tag on first line.
        let after_lang = rest
            .splitn(2, '\n')
            .nth(1)
            .unwrap_or(rest);
        if let Some(end) = after_lang.rfind("```") {
            return after_lang[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn extract_balanced(s: &str) -> String {
    // Find the largest substring starting with `{` or `[` and ending with
    // its matching close.
    let bytes = s.as_bytes();
    let mut best: Option<(usize, usize)> = None;

    for (start, &b) in bytes.iter().enumerate() {
        if b != b'{' && b != b'[' {
            continue;
        }
        let open = b;
        let close = if b == b'{' { b'}' } else { b']' };
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;
        for (i, &c) in bytes.iter().enumerate().skip(start) {
            if in_string {
                if escape {
                    escape = false;
                } else if c == b'\\' {
                    escape = true;
                } else if c == b'"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                b'"' => in_string = true,
                x if x == open => depth += 1,
                x if x == close => {
                    depth -= 1;
                    if depth == 0 {
                        let len = i - start + 1;
                        if best.map(|(s0, e0)| (e0 - s0) < len).unwrap_or(true) {
                            best = Some((start, i + 1));
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    match best {
        Some((a, b)) => s[a..b].to_string(),
        None => s.to_string(),
    }
}

fn strip_trailing_commas(s: &str) -> String {
    // Remove `,` followed by optional whitespace then `}` or `]`.
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut escape = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c as char);
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b',' {
            // Look ahead past whitespace; if next non-ws is } or ], skip the comma.
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b'}' || bytes[j] == b']') {
                i += 1; // drop the comma
                continue;
            }
        }
        out.push(c as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_fence_with_language() {
        let s = "```json\n{\"a\": 1}\n```";
        assert_eq!(repair(s).trim(), "{\"a\": 1}");
    }

    #[test]
    fn strips_markdown_fence_without_language() {
        let s = "```\n[1,2,3]\n```";
        assert_eq!(repair(s).trim(), "[1,2,3]");
    }

    #[test]
    fn extracts_object_from_prose() {
        let s = "Sure, here you go: {\"a\": 1} hope that helps!";
        assert_eq!(repair(s).trim(), "{\"a\": 1}");
    }

    #[test]
    fn removes_trailing_commas() {
        assert_eq!(repair("{\"a\": 1,}"), "{\"a\": 1}");
        assert_eq!(repair("[1, 2, 3,]"), "[1, 2, 3]");
    }

    #[test]
    fn doesnt_break_strings_with_commas_or_braces() {
        let s = r#"{"text": "hello, world"}"#;
        assert_eq!(repair(s), s);
    }

    #[test]
    fn nested_extraction() {
        let s = "Output: {\"outer\": {\"inner\": [1,2]}, \"arr\": [3,]}";
        let r = repair(s);
        assert!(r.contains("\"outer\""));
        assert!(!r.contains("3,]"));
    }
}
