//! Numbers, dates and paths as the UI shows them.

use chrono::{DateTime, Datelike, Local};
use std::path::Path;

/// 1234567 -> "1,234,567" (07.3: separators on every number).
pub fn num(n: impl Into<i64>) -> String {
    let n: i64 = n.into();
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "3 values", "1 value".
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", num(n as i64), if n == 1 { one } else { many })
}

/// Play time in seconds as "48h 43m".
pub fn playtime(secs: u32) -> String {
    format!("{}h {:02}m", secs / 3600, secs / 60 % 60)
}

/// "Today, 14:32", "Yesterday, 21:05", "3 Oct, 18:40", "3 Oct 2025, 18:40".
pub fn when(t: DateTime<Local>) -> String {
    let now = Local::now();
    let days = now.date_naive().signed_duration_since(t.date_naive()).num_days();
    let time = t.format("%H:%M");
    match days {
        0 => format!("Today, {time}"),
        1 => format!("Yesterday, {time}"),
        _ if t.year() == now.year() => format!("{} {}, {time}", t.day(), t.format("%b")),
        _ => format!("{} {} {}, {time}", t.day(), t.format("%b"), t.year()),
    }
}

/// A long path with whole folders dropped from the middle, so its start and the part
/// that differs stay readable: "/home/u/…/0000000000000001/1/system_backup" (02.4).
pub fn elide_path(p: &str, max: usize) -> String {
    if p.chars().count() <= max {
        return p.to_string();
    }
    let sep = if p.contains('\\') && !p.contains('/') { '\\' } else { '/' };
    let parts: Vec<&str> = p.split(sep).collect();
    let keep = 3.min(parts.len());
    let head = parts[..keep].join(&sep.to_string());
    let mut tail: Vec<&str> = vec![];
    let mut len = head.chars().count() + 2;
    for part in parts[keep..].iter().rev() {
        len += part.chars().count() + 1;
        if len > max && !tail.is_empty() {
            break;
        }
        tail.push(part);
    }
    if tail.len() == parts.len() - keep {
        return p.to_string();
    }
    tail.reverse();
    format!("{head}{sep}…{sep}{}", tail.join(&sep.to_string()))
}

/// The last `n` components, elided at the start: "…/save/0000000000000001/0/system" (K2).
pub fn path_tail(p: &Path, n: usize) -> String {
    let parts: Vec<String> = p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.len() <= n + 1 {
        return p.display().to_string();
    }
    format!("…/{}", parts[parts.len() - n..].join("/"))
}

/// "A, B and C"; past `max` names "A, B and 3 more".
pub fn list(names: &[String], max: usize) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        n if n <= max => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]),
        n => format!("{} and {} more", names[..max - 1].join(", "), n - (max - 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(num(0), "0");
        assert_eq!(num(999), "999");
        assert_eq!(num(1000), "1,000");
        assert_eq!(num(9_999_999), "9,999,999");
        assert_eq!(num(-1234), "-1,234");
        assert_eq!(count(1, "value", "values"), "1 value");
        assert_eq!(count(1255, "stack", "stacks"), "1,255 stacks");
    }

    #[test]
    fn paths() {
        let p = Path::new("/home/u/.config/Ryujinx/bis/user/save/0000000000000001/0/system");
        assert_eq!(path_tail(p, 4), "…/save/0000000000000001/0/system");
        let e = elide_path("/home/u/.config/Ryujinx/bis/user/save/0000000000000001/1/system_backup", 50);
        assert_eq!(e, "/home/u/…/save/0000000000000001/1/system_backup");
        assert_eq!(elide_path("/home/u/a", 50), "/home/u/a");
        assert_eq!(list(&["A".into(), "B".into(), "C".into()], 3), "A, B and C");
        assert_eq!(list(&["A".into(), "B".into(), "C".into(), "D".into()], 3), "A, B and 2 more");
    }
}
