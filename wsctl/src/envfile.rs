//! Reading and writing the `.env` the stack is configured from.
//!
//! Values are patched into the existing file rather than the file being rewritten, so the
//! comments explaining each setting survive, and anything the operator added by hand stays
//! where they put it.

use std::collections::BTreeMap;

/// Replace the values of matching `KEY=...` lines and keep everything else verbatim. Keys
/// the file does not already have are appended.
pub fn patch(current: &str, values: &BTreeMap<String, String>) -> String {
    let mut remaining = values.clone();
    let mut out = String::new();

    for line in current.lines() {
        let patched = key_of(line)
            .and_then(|key| remaining.remove(key).map(|value| (key, value)))
            .map(|(key, value)| format!("{key}={}", quote(&value)));
        out.push_str(patched.as_deref().unwrap_or(line));
        out.push('\n');
    }

    if !remaining.is_empty() {
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str("# Added by wsctl\n");
        for (key, value) in &remaining {
            out.push_str(&format!("{key}={}\n", quote(value)));
        }
    }

    out
}

/// The value of a `KEY=...` line, with surrounding quotes removed.
pub fn get(env: &str, key: &str) -> Option<String> {
    env.lines().find_map(|line| {
        if line.trim_start().starts_with('#') {
            return None;
        }
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| unquote(v.trim()))
    })
}

/// Drop a key entirely. Discord is read as all-or-nothing by the server, so an empty value
/// left behind would read as "configured" and stop it starting.
pub fn remove(current: &str, key: &str) -> String {
    current
        .lines()
        .filter(|line| key_of(line) != Some(key))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(key)
}

/// Undo `quote`. A backslash escapes whatever follows it, which is what dotenv does inside
/// double quotes, so a value holding one survives the trip out and back.
fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return trimmed.to_string();
    }
    let mut out = String::new();
    let mut chars = trimmed[1..trimmed.len() - 1].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(chars.next().unwrap_or('\\')),
            other => out.push(other),
        }
    }
    out
}

/// Quote anything that would not survive a bare `KEY=value` line. dotenv rejects an
/// unquoted value with a space in it, and a character name is two words.
fn quote(value: &str) -> String {
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || "|#\"'$`\\".contains(c))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn replaces_a_value_and_leaves_the_comments_alone() {
        let current = "# what this is for\nWS_DOMAIN=old.example.com\nHTTP_PORT=80\n";
        let out = patch(current, &values(&[("WS_DOMAIN", "map.example.com")]));
        assert_eq!(
            out,
            "# what this is for\nWS_DOMAIN=map.example.com\nHTTP_PORT=80\n"
        );
    }

    // A character name is two words, and dotenv refuses a bare value with a space in it.
    #[test]
    fn quotes_anything_a_bare_line_would_not_survive() {
        let out = patch(
            "WS_CONTACT_NAME=\n",
            &values(&[("WS_CONTACT_NAME", "Nicolas Kion")]),
        );
        assert_eq!(out, "WS_CONTACT_NAME=\"Nicolas Kion\"\n");
    }

    #[test]
    fn escapes_a_quote_inside_a_value() {
        let out = patch("K=\n", &values(&[("K", "a\"b")]));
        assert_eq!(out, "K=\"a\\\"b\"\n");
        assert_eq!(get(&out, "K").as_deref(), Some("a\"b"));
    }

    #[test]
    fn appends_keys_the_file_does_not_have_yet() {
        let out = patch("EXISTING=1\n", &values(&[("NEW", "2")]));
        assert!(out.starts_with("EXISTING=1\n"));
        assert!(out.contains("# Added by wsctl\nNEW=2\n"));
    }

    #[test]
    fn reads_plain_and_quoted_values_and_ignores_comments() {
        let env = "# WS_DOMAIN=commented.example.com\nWS_DOMAIN=\"map.example.com\"\nPORT=80\n";
        assert_eq!(get(env, "WS_DOMAIN").as_deref(), Some("map.example.com"));
        assert_eq!(get(env, "PORT").as_deref(), Some("80"));
        assert_eq!(get(env, "ABSENT"), None);
    }

    #[test]
    fn removing_a_key_takes_the_whole_line() {
        let env = "A=1\nDISCORD_CLIENT_ID=abc\nB=2\n";
        assert_eq!(remove(env, "DISCORD_CLIENT_ID"), "A=1\nB=2\n");
    }

    // Round-tripping is the property that matters: whatever is written can be read back.
    #[test]
    fn every_value_survives_a_round_trip() {
        for value in [
            "simple",
            "two words",
            "with#hash",
            "with$dollar",
            "with\"quote",
            "with\\backslash",
            "",
        ] {
            let out = patch("K=\n", &values(&[("K", value)]));
            assert_eq!(get(&out, "K").as_deref(), Some(value), "value {value:?}");
        }
    }
}
