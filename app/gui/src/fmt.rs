//! Numbers, dates and paths as the UI shows them.

use crate::i18n::{tr, trf};
use chrono::{DateTime, Datelike, Local};
use std::path::Path;

/// 1234567 -> "1,234,567" (07.3: separators on every number), with the language's
/// separator.
pub fn num(n: impl Into<i64>) -> String {
    let n: i64 = n.into();
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(crate::i18n::thousands());
        }
        out.push(c);
    }
    out
}

/// Play time in seconds as "48h 43m".
pub fn playtime(secs: u32) -> String {
    trf("{}h {}m", &[&(secs / 3600), &format!("{:02}", secs / 60 % 60)])
}

/// An Arena time in frames as the game shows it, "4:05.30".
pub fn arena_time(frames: u32) -> String {
    let t = mhgu_save::arena::hundredths(frames);
    format!("{}:{:02}.{:02}", t / 6000, t / 100 % 60, t % 100)
}

/// "30 Aug 2026"; a date the game never wrote shows as a dash.
pub fn date(day: u8, m: u8, year: u16) -> String {
    if day == 0 || !(1..=12).contains(&m) {
        return "—".into();
    }
    // day, month, year
    trf("{0} {1} {2}", &[&day, &month(m as u32), &year])
}

/// Short month name, 1 = January.
fn month(m: u32) -> &'static str {
    match m {
        1 => tr("Jan"),
        2 => tr("Feb"),
        3 => tr("Mar"),
        4 => tr("Apr"),
        5 => tr("May"),
        6 => tr("Jun"),
        7 => tr("Jul"),
        8 => tr("Aug"),
        9 => tr("Sep"),
        10 => tr("Oct"),
        11 => tr("Nov"),
        _ => tr("Dec"),
    }
}

/// "Today, 14:32", "Yesterday, 21:05", "3 Oct, 18:40", "3 Oct 2025, 18:40".
pub fn when(t: DateTime<Local>) -> String {
    let now = Local::now();
    let days = now.date_naive().signed_duration_since(t.date_naive()).num_days();
    let time = t.format("%H:%M").to_string();
    match days {
        0 => trf("Today, {}", &[&time]),
        1 => trf("Yesterday, {}", &[&time]),
        // day, month, time
        _ if t.year() == now.year() => trf("{0} {1}, {2}", &[&t.day(), &month(t.month()), &time]),
        // day, month, year, time
        _ => trf("{0} {1} {2}, {3}", &[&t.day(), &month(t.month()), &t.year(), &time]),
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
        n if n <= max => trf("{} and {}", &[&names[..n - 1].join(", "), &names[n - 1]]),
        n => trf("{} and {} more", &[&names[..max - 1].join(", "), &num((n - (max - 1)) as i64)]),
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
