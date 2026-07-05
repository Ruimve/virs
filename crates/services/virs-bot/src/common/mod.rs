//! Shared types, ports, AI client, and indicators for bot services.

pub mod ai_client;
pub mod indicators;
pub mod ports;

/// Truncate a string to `max_len` characters, appending "..." if truncated.
/// Used for logging fields that may contain arbitrarily long LLM output.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Find a safe UTF-8 boundary
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
