use chrono::{DateTime, Duration, Local};
use std::path::Path;

pub fn compress_text(text: &str, max_length: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return String::new();
    }
    if clean.chars().count() <= max_length {
        return clean;
    }
    if max_length <= 3 {
        return ".".repeat(max_length);
    }
    let prefix = clean.chars().take(max_length - 3).collect::<String>();
    format!("{prefix}...")
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

pub fn format_count(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
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

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64 / 1024.0;
    let mut unit = UNITS[0];
    for next_unit in UNITS.iter().skip(1) {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next_unit;
    }

    if value >= 10.0 || value.fract() < 0.05 {
        format!("{value:.0} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
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

#[cfg(test)]
mod tests {
    use super::{compress_text, format_bytes};

    #[test]
    fn truncates_multilingual_titles_without_splitting_utf8() {
        assert_eq!(compress_text("修复登录问题和测试", 7), "修复登录...");
        assert_eq!(compress_text("ship 🚀 release", 10), "ship 🚀 ...");
        assert_eq!(compress_text("café", 4), "café");
    }

    #[test]
    fn formats_storage_sizes_compactly() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(10 * 1024), "10 KiB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2 MiB");
    }
}
