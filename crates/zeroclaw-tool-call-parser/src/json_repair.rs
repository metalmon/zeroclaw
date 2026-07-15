//! Conservative JSON-object repair for native tool-call argument strings (#8675).

use serde_json::Value;

pub fn repair_json_object_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if parses_as_object(trimmed) {
        return Some(trimmed.to_string());
    }

    for candidate in [
        trimmed.to_string(),
        strip_trailing_commas(trimmed),
        close_unclosed_json(trimmed),
        close_unclosed_json(&strip_trailing_commas(trimmed)),
    ] {
        if parses_as_object(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn parses_as_object(s: &str) -> bool {
    serde_json::from_str::<Value>(s)
        .ok()
        .is_some_and(|v| v.is_object())
}

fn strip_trailing_commas(input: &str) -> String {
    let mut s = input.trim().to_string();
    loop {
        let before = s.clone();
        while s.ends_with(',') {
            s.pop();
            s = s.trim_end().to_string();
        }
        s = remove_commas_before_closers(&s);
        if s == before {
            break;
        }
    }
    s
}

fn remove_commas_before_closers(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        if in_string {
            out.push(ch);
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }
        if ch == ',' {
            let mut peek = chars.clone();
            let next = loop {
                match peek.next() {
                    Some(c) if c.is_whitespace() => {}
                    other => break other,
                }
            };
            if next == Some('}') || next == Some(']') {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn close_unclosed_json(input: &str) -> String {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                if stack.last() == Some(&ch) {
                    stack.pop();
                }
            }
            _ => {}
        }
    }

    let mut s = input.trim().to_string();
    if in_string {
        s.push('"');
    }
    if s.ends_with(':') {
        s.push_str("null");
    }
    while let Some(closer) = stack.pop() {
        s.push(closer);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::repair_json_object_string;

    #[test]
    fn repair_strips_trailing_comma_before_close() {
        assert_eq!(
            repair_json_object_string(r#"{"a": 1,}"#).as_deref(),
            Some(r#"{"a": 1}"#)
        );
    }

    #[test]
    fn repair_closes_unclosed_string_and_object() {
        assert_eq!(
            repair_json_object_string(r#"{"path": "unclosed"#).as_deref(),
            Some(r#"{"path": "unclosed"}"#)
        );
    }

    #[test]
    fn repair_strips_trailing_comma_at_eof() {
        assert_eq!(
            repair_json_object_string(r#"{"command": "pwd","#).as_deref(),
            Some(r#"{"command": "pwd"}"#)
        );
    }

    #[test]
    fn repair_rejects_non_object_and_garbage() {
        assert!(repair_json_object_string("not json").is_none());
        assert!(repair_json_object_string("[1,2]").is_none());
    }

    #[test]
    fn repair_passes_through_valid_object() {
        let raw = r#"{"path":"foo.txt"}"#;
        assert_eq!(repair_json_object_string(raw).as_deref(), Some(raw));
    }
}
