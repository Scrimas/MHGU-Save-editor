//! Self update from GitHub: the latest release (the API leaves out drafts and
//! pre-releases) is compared with this version. Only when the user asks, its AppImage or
//! .exe is downloaded, checked against the release's SHA256SUMS and put in place of the
//! running file, renamed to the new version when the old name held the old one.
//! Windows cannot delete a running .exe: it is renamed aside and deleted on the next start.
//!
//! The checksums come from the same release as the file: they catch a broken download,
//! not a release published by someone else (there is no signature).

use crate::i18n::{tr, trf};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/Scrimas/MHGU-Save-editor/releases/latest";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(windows)]
const SUFFIX: &str = "-x86_64.exe";
#[cfg(not(windows))]
const SUFFIX: &str = "-x86_64.AppImage";

#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Deserialize)]
struct Latest {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}

/// A release newer than this one.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    /// Its page on GitHub, with the notes.
    pub page: String,
    file: Option<Asset>,
    sums: Option<Asset>,
}

impl Release {
    /// Download size, when this platform's file is in the release.
    pub fn size(&self) -> Option<u64> {
        self.file.as_ref().map(|a| a.size)
    }
}

/// "v1.10.2" -> (1, 10, 2, true), for comparing; "1.0" is 1.0.0 and a pre-release
/// ("1.0.0-rc.1") comes before its release. Anything else is None, older than all.
pub fn version(v: &str) -> Option<(u32, u32, u32, bool)> {
    let v = v.trim().trim_start_matches('v');
    let v = v.split_once('+').map_or(v, |(a, _)| a);
    let (core, release) = match v.split_once('-') {
        Some((c, _)) => (c, false),
        None => (v, true),
    };
    let n: Vec<u32> = core.split('.').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    match n[..] {
        [a] => Some((a, 0, 0, release)),
        [a, b] => Some((a, b, 0, release)),
        [a, b, c] => Some((a, b, c, release)),
        _ => None,
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        // a stalled download fails instead of waiting forever (Cancel also leaves)
        .timeout_recv_body(Some(Duration::from_secs(15 * 60)))
        .user_agent(format!("mhgu-save-editor/{VERSION}"))
        .build()
        .into()
}

fn get(url: &str) -> Result<ureq::Body, String> {
    match agent().get(url).call() {
        Ok(r) => Ok(r.into_body()),
        Err(ureq::Error::StatusCode(403 | 429)) => Err(tr("GitHub refused the request (too many in the last hour), try later").into()),
        Err(ureq::Error::StatusCode(c)) => Err(trf("GitHub answered HTTP {}", &[&c])),
        Err(e) => Err(trf("Could not reach GitHub ({})", &[&e])),
    }
}

/// The newest release when it is newer than this one.
pub fn check() -> Result<Option<Release>, String> {
    let r = match fetch(LATEST) {
        // no published release yet (`get`'s message for a 404, in the language in use)
        Err(e) if e == trf("GitHub answered HTTP {}", &[&404]) => return Ok(None),
        r => r?,
    };
    Ok(Some(r).filter(|r| version(&r.version) > version(VERSION)))
}

/// The checksum list of a release, by its exact name (not a `.sig` beside it):
/// `SHA256SUMS-<version>` as build.sh names it, or `SHA256SUMS`, the copy releases carry
/// for 0.8.0, which looks for that name only.
const SUMS: &str = "SHA256SUMS";

fn is_sums(name: &str, version: &str) -> bool {
    name == SUMS || name.strip_prefix(SUMS).and_then(|r| r.strip_prefix('-')) == Some(version)
}

/// What `install` did: the new file, and on Windows the old .exe renamed aside (to
/// delete on the next start).
pub struct Installed {
    pub file: PathBuf,
    pub aside: Option<PathBuf>,
}

fn fetch(url: &str) -> Result<Release, String> {
    let l: Latest = serde_json::from_reader(get(url)?.into_reader()).map_err(|e| trf("Unexpected answer from GitHub ({})", &[&e]))?;
    let pick = |f: &dyn Fn(&str) -> bool| l.assets.iter().find(|a| f(&a.name)).cloned();
    let version = l.tag_name.trim_start_matches('v').to_string();
    Ok(Release {
        page: l.html_url.clone(),
        file: pick(&|n| n.ends_with(SUFFIX)),
        sums: pick(&|n| is_sums(n, &version)),
        version,
    })
}

/// The file an update replaces: the AppImage or the .exe; None when run another way.
pub fn target() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::current_exe().ok();
    #[cfg(not(windows))]
    std::env::var_os("APPIMAGE").map(PathBuf::from)
}

/// The updated file's name: the version in `name` becomes `to`; a name without it stays.
fn renamed(name: &str, to: &str) -> String {
    name.replacen(&format!("-{VERSION}-"), &format!("-{to}-"), 1)
}

/// The hash of `name` in a `sha256sum` listing.
fn sum_of(list: &str, name: &str) -> Option<String> {
    list.lines().find_map(|l| {
        let (h, n) = l.split_once(char::is_whitespace)?;
        (n.trim_start().trim_start_matches('*') == name).then(|| h.to_ascii_lowercase())
    })
}

/// Download, check and swap in the new file for `old` (the `target`);
/// `progress(done, total)` is called as it comes; `cancel` stops the download.
pub fn install(r: &Release, old: &Path, progress: impl Fn(u64, u64), cancel: &AtomicBool) -> Result<Installed, String> {
    let (Some(file), Some(sums)) = (&r.file, &r.sums) else {
        return Err(trf("Release {} has no {} file with checksums", &[&r.version, &SUFFIX]));
    };
    let mut list = String::new();
    get(&sums.browser_download_url)?.into_reader().read_to_string(&mut list).map_err(|e| trf("Download failed ({})", &[&e]))?;
    let want = sum_of(&list, &file.name).ok_or_else(|| trf("{} is not in {}", &[&file.name, &sums.name]))?;

    let name = old.file_name().and_then(|n| n.to_str()).ok_or(tr("The running file has no usable name"))?;
    let new = old.with_file_name(renamed(name, &r.version));
    let part = old.with_file_name(format!("{}.part", renamed(name, &r.version)));
    let res = download(&file.browser_download_url, &part, file.size, &progress, cancel).and_then(|got| {
        if got != want {
            return Err(tr("The download does not match the release's checksum").into());
        }
        swap(old, &part, &new)
    });
    if res.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    res.map(|aside| Installed { file: new, aside })
}

fn download(url: &str, to: &Path, size: u64, progress: &impl Fn(u64, u64), cancel: &AtomicBool) -> Result<String, String> {
    let err = |e: std::io::Error| trf("Could not write {} ({})", &[&to.display(), &e]);
    let mut body = get(url)?.into_reader();
    let mut out = std::fs::File::create(to).map_err(err)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    let mut done = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(tr("Cancelled").into());
        }
        let n = body.read(&mut buf).map_err(|e| trf("Download failed ({})", &[&e]))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(err)?;
        done += n as u64;
        progress(done, size);
    }
    out.sync_all().map_err(err)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        out.set_permissions(std::fs::Permissions::from_mode(0o755)).map_err(err)?;
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The new file takes the old one's place. A running AppImage can be deleted (it stays
/// mounted until it exits); a running .exe only renamed: returned, to delete later.
fn swap(old: &Path, part: &Path, new: &Path) -> Result<Option<PathBuf>, String> {
    let err = |e: std::io::Error| trf("Could not replace {} ({})", &[&old.display(), &e]);
    if cfg!(windows) {
        let aside = old.with_file_name(format!("{}.old", old.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_file(&aside);
        std::fs::rename(old, &aside).map_err(err)?;
        if let Err(e) = std::fs::rename(part, new) {
            let _ = std::fs::rename(&aside, old);
            return Err(err(e));
        }
        Ok(Some(aside))
    } else {
        std::fs::rename(part, new).map_err(err)?;
        if new != old {
            let _ = std::fs::remove_file(old);
        }
        Ok(None)
    }
}

/// On start: delete the .exe an update left aside.
pub fn cleanup() {
    if let Some(p) = crate::settings::get().update_leftover
        && (std::fs::remove_file(&p).is_ok() || !p.exists())
    {
        crate::settings::update(|s| s.update_leftover = None);
    }
}

/// Start the updated file, on the open save if any.
pub fn restart(exe: &Path, save: Option<&Path>) -> std::io::Result<()> {
    let mut c = std::process::Command::new(exe);
    c.args(save);
    // the AppImage runtime's variables describe this AppImage, not the new one
    for v in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        c.env_remove(v);
    }
    c.spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(version("v0.10.0") > version("0.9.1"));
        assert!(version("1.0") < version("1.0.1"));
        assert_eq!(version("v1.2.3"), version("1.2.3"));
        assert_eq!(version("1.0"), version("1.0.0"));
        assert!(version("1.0.0-rc.1") < version("1.0.0"));
        assert!(version("1.0.0-rc.1") > version("0.9.9"));
        assert_eq!(version("1.0.0+build.5"), version("1.0.0"));
        assert_eq!(version("latest"), None);
        assert!(version("garbage") < version("0.0.1"), "an unreadable tag is never newer");
    }

    #[test]
    fn the_version_in_the_name_follows() {
        assert_eq!(renamed(&format!("MHGU-Save-Editor-{VERSION}-x86_64.AppImage"), "9.9.9"), "MHGU-Save-Editor-9.9.9-x86_64.AppImage");
        assert_eq!(renamed("mhgu.exe", "9.9.9"), "mhgu.exe");
    }

    #[test]
    fn checksums_are_found_by_name() {
        let list = "AAA  MHGU-Save-Editor-1.0.0-x86_64.AppImage\nbbb *MHGU-Save-Editor-1.0.0-x86_64.exe\n";
        assert_eq!(sum_of(list, "MHGU-Save-Editor-1.0.0-x86_64.AppImage").as_deref(), Some("aaa"));
        assert_eq!(sum_of(list, "MHGU-Save-Editor-1.0.0-x86_64.exe").as_deref(), Some("bbb"));
        assert_eq!(sum_of(list, "other"), None);
        // the list itself: as build.sh names it, or the plain copy
        assert!(is_sums("SHA256SUMS-0.8.0", "0.8.0") && is_sums("SHA256SUMS", "0.8.0"));
        assert!(!is_sums("SHA256SUMS-0.7.0", "0.8.0") && !is_sums("SHA256SUMS-0.8.0.sig", "0.8.0") && !is_sums("SHA256SUMS.sig", "0.8.0"));
    }

    #[test]
    fn swap_replaces_the_old_file() {
        let d = std::env::temp_dir().join(format!("mhgu-update-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (old, part, new) = (d.join("a-1-x"), d.join("a-2-x.part"), d.join("a-2-x"));
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&part, "new").unwrap();
        let aside = swap(&old, &part, &new).unwrap();
        assert_eq!(aside.is_some(), cfg!(windows));
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "new");
        assert!(!part.exists());
        assert!(!old.exists() || cfg!(windows));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Downloads v0.5.0 from GitHub: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn installs_a_published_release() {
        let r = fetch("https://api.github.com/repos/Scrimas/MHGU-Save-editor/releases/tags/v0.5.0").unwrap();
        let d = std::env::temp_dir().join(format!("mhgu-install-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let old = d.join(format!("MHGU-Save-Editor-{VERSION}{SUFFIX}"));
        std::fs::write(&old, "old").unwrap();
        let last = std::cell::Cell::new(0);
        let new = install(
            &r,
            &old,
            |done, total| {
                assert!(done > last.get() && done <= total);
                last.set(done);
            },
            &AtomicBool::new(false),
        )
        .unwrap()
        .file;
        assert_eq!(new, d.join(format!("MHGU-Save-Editor-0.5.0{SUFFIX}")));
        assert_eq!(std::fs::metadata(&new).unwrap().len(), r.size().unwrap());
        assert!(!old.exists());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
