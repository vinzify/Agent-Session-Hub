use chrono::{DateTime, Duration, Local};
use std::path::Path;

pub fn compress_text(text: &str, max_length: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return String::new();
    }
    if clean.len() <= max_length {
        return clean;
    }
    if max_length <= 3 {
        return ".".repeat(max_length);
    }
    format!("{}...", &clean[..max_length - 3])
}

pub fn project_name(project_path: &str) -> String {
    if project_path.trim().is_empty() {
        return "<unknown>".to_string();
    }
    Path::new(project_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| project_path.to_string())
}

pub fn format_timestamp(timestamp: DateTime<Local>) -> String {
    timestamp.format("%Y-%m-%d %H:%M").to_string()
}

pub fn format_relative_age(timestamp: DateTime<Local>) -> String {
    let now = Local::now();
    let delta = now - timestamp;

    if delta < Duration::minutes(1) {
        return format!("{}s ago", delta.num_seconds().max(1));
    }
    if delta < Duration::hours(1) {
        return format!("{}m ago", delta.num_minutes());
    }
    if delta < Duration::days(1) {
        return format!("{}h ago", delta.num_hours());
    }
    format!("{}d ago", delta.num_days())
}

pub fn ascii_banner(kind: &str, primary: &str, secondary: &str) -> String {
    let kind_text = if kind.trim().is_empty() {
        "ITEM".to_string()
    } else {
        kind.to_ascii_uppercase()
    };
    let primary = if primary.trim().is_empty() {
        "-".to_string()
    } else {
        compress_text(primary, 36)
    };
    let secondary = if secondary.trim().is_empty() {
        String::new()
    } else {
        compress_text(secondary, 18)
    };
    let headline = if secondary.is_empty() {
        format!("{kind_text} | {primary}")
    } else {
        format!("{kind_text} | {primary} | {secondary}")
    };
    let border = format!("+{}+", "-".repeat(headline.len()));
    format!("{border}\n|{headline}|\n{border}")
}

pub fn escape_sh_single_quotes(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}
