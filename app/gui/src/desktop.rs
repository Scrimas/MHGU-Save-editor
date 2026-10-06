//! Run as an AppImage, the editor puts itself in the application menu: a desktop entry
//! pointing at the AppImage and the icons, in the user's data folder
//! (`~/.local/share/applications`, `~/.local/share/icons/hicolor`). The newest version
//! keeps the entry: an older AppImage takes it only when the one it names is gone.
//! `TryExec` hides the entry once that AppImage is deleted.

use crate::update::{version, VERSION};
use std::path::Path;

const NAME: &str = "mhgu-save-editor";
const ENTRY: &str = include_str!("../../packaging/mhgu-save-editor.desktop");
const PNG: &[u8] = include_bytes!("../assets/icon-256.png");
const SVG: &[u8] = include_bytes!("../assets/icon.svg");

/// Write or refresh the entry when started from an AppImage; nothing otherwise.
pub fn install() {
    let Some(appimage) = std::env::var_os("APPIMAGE") else { return };
    let Some(data) = dirs::data_dir() else { return };
    let Some(path) = Path::new(&appimage).to_str().filter(|p| p.starts_with('/') && !p.contains('\n')) else {
        return;
    };
    let file = data.join("applications").join(format!("{NAME}.desktop"));
    if let Ok(old) = std::fs::read_to_string(&file) {
        let alive = field(&old, "TryExec").is_some_and(|p| Path::new(&unescape(p)).is_file());
        let newer = field(&old, "X-AppImage-Version").is_some_and(|v| version(v) > version(VERSION));
        if alive && newer {
            return;
        }
    }
    let icons = data.join("icons/hicolor");
    for (p, b) in [
        (file, entry(path).into_bytes()),
        (icons.join(format!("256x256/apps/{NAME}.png")), PNG.to_vec()),
        (icons.join(format!("scalable/apps/{NAME}.svg")), SVG.to_vec()),
    ] {
        if let Err(e) = write(&p, &b) {
            eprintln!("desktop entry: {}: {e}", p.display());
        }
    }
}

/// The packaged entry with `Exec` on the AppImage.
fn entry(appimage: &str) -> String {
    let mut out = String::new();
    for l in ENTRY.lines() {
        if l.starts_with("Exec=") {
            out += &format!("Exec={} %f\nTryExec={}\n", quote(appimage), appimage.replace('\\', "\\\\"));
        } else {
            out += l;
            out += "\n";
        }
    }
    out + &format!("X-AppImage-Version={VERSION}\n")
}

/// An `Exec` argument: double quotes with `"` `` ` `` `$` `\` escaped, then the string
/// escape doubles every backslash, and `%` is doubled for field codes.
fn quote(s: &str) -> String {
    let mut q = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '`' | '$' => q += &format!("\\\\{c}"),
            '\\' => q += "\\\\\\\\",
            '%' => q += "%%",
            _ => q.push(c),
        }
    }
    q + "\""
}

fn unescape(s: &str) -> String {
    s.replace("\\\\", "\\")
}

fn field<'a>(entry: &'a str, key: &str) -> Option<&'a str> {
    entry.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
}

/// Write only when different, through a temporary file so the desktop never reads half.
fn write(p: &Path, b: &[u8]) -> std::io::Result<()> {
    if std::fs::read(p).is_ok_and(|old| old == b) {
        return Ok(());
    }
    std::fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, b)?;
    std::fs::rename(&tmp, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_points_at_the_appimage() {
        let e = entry("/home/a b/MHGU 100%.AppImage");
        assert!(e.contains("Exec=\"/home/a b/MHGU 100%%.AppImage\" %f\n"));
        assert_eq!(field(&e, "TryExec"), Some("/home/a b/MHGU 100%.AppImage"));
        assert_eq!(field(&e, "X-AppImage-Version"), Some(VERSION));
        assert_eq!(e.matches("Exec=").count(), 2);
    }

    #[test]
    fn reserved_characters_are_escaped() {
        assert_eq!(quote(r#"/a"$`\b"#), r#""/a\\"\\$\\`\\\\b""#);
        assert_eq!(unescape(&entry(r"/a\b").lines().find(|l| l.starts_with("TryExec=")).unwrap()[8..]), r"/a\b");
    }
}
