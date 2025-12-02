use lib_utils::format_message;

pub fn parse_int(s: &str) -> Result<i32, String> {
    s.parse().map_err(|e| format_message(&format!("Parse error: {}", e)))
}

pub fn split_words(s: &str) -> Vec<String> {
    s.split_whitespace().map(|w| w.to_string()).collect()
}

pub fn count_tokens(s: &str) -> usize {
    split_words(s).len()
}

