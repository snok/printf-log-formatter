use crate::enums::LogLevel;

/// Combine all selected log-levels into a regex pattern
pub fn construct_f_string_regex(log_level: &LogLevel) -> String {
    let mut regex_str = r#"logger\.("#.to_string();
    regex_str.push_str(&LogLevel::regex_strings_above_log_level(log_level));
    regex_str.push_str(r#")\(f"(.+?)"\)"#);
    regex_str
}

pub fn replace_last_char_with_str(mut s: String, replacement: &str) -> String {
    if let Some((start, end)) = s.char_indices().rev().next() {
        let char_len = end.len_utf8();
        let end_idx = start + char_len;
        s.replace_range(start..end_idx, replacement);
    }
    s
}
