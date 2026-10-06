//! Interface languages. The .slint files' @tr() strings and the Rust side's tr() share one
//! gettext catalog per language, app/gui/lang/<code>/LC_MESSAGES/mhgu-editor.po (kept up to
//! date by tools/i18n.py): Slint bundles them at build time, tr() parses them at first use.
//! English is the source text. Game names (items, gear, monsters) come from the asset pack
//! in the game's own languages instead (assets.rs).
//!
//! Placeholders as in Slint: `{}` in order, `{0}` `{1}` … by position, `{n}` the count of a
//! plural, `{{` `}}` braces.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The lang/ folder (a gettext locale) and the language's own name, English first: the
/// game's own languages, so its names (assets.rs) match the interface.
pub const LANGS: [(&str, &str); 5] = [("en", "English"), ("fr", "Français"), ("de", "Deutsch"), ("es", "Español"), ("it", "Italiano")];

macro_rules! po {
    ($c:literal) => {
        include_str!(concat!("../lang/", $c, "/LC_MESSAGES/mhgu-editor.po"))
    };
}

/// The catalogs in LANGS order (English has none).
const PO: [&str; 5] = ["", po!("fr"), po!("de"), po!("es"), po!("it")];

/// Plural form of count `n` per language, as in the catalogs' Plural-Forms header.
fn plural(lang: usize, n: i64) -> usize {
    match LANGS[lang].0 {
        "fr" => (n > 1) as usize,
        _ => (n != 1) as usize,
    }
}

static LANG: AtomicUsize = AtomicUsize::new(0);

/// Index of the language in use (LANGS).
pub fn current() -> usize {
    LANG.load(Ordering::Relaxed)
}

/// The LANGS index for setting `code` ("" for the system's language).
pub fn index(code: &str) -> usize {
    if code.is_empty() {
        return system();
    }
    LANGS.iter().position(|l| l.0 == code).unwrap_or(0)
}

/// The system's language, matched as Slint does: the whole locale, then its language part.
pub fn system() -> usize {
    let Some(loc) = sys_locale::get_locale() else { return 0 };
    let loc = loc.replace('-', "_");
    let base = |l: &str| l.split(['_', '@', '.']).next().unwrap_or("").to_string();
    LANGS
        .iter()
        .position(|l| l.0 == loc)
        .or_else(|| LANGS.iter().position(|l| base(l.0) == base(&loc)))
        .unwrap_or(0)
}

/// The thousands separator of the language in use (a no-break space in French).
pub fn thousands() -> &'static str {
    match LANGS[current()].0 {
        "en" => ",",
        "fr" => "\u{a0}",
        _ => ".",
    }
}

/// Switch the .slint strings, their numbers and tr() to language `i`.
pub fn set(ui: &crate::AppWindow, i: usize) {
    use slint::ComponentHandle;
    let i = i.min(LANGS.len() - 1);
    LANG.store(i, Ordering::Relaxed);
    if let Err(e) = slint::select_bundled_translation(LANGS[i].0) {
        eprintln!("language {}: {e}", LANGS[i].0);
    }
    ui.global::<crate::Fmt>().set_sep(thousands().into());
}

#[derive(Default)]
struct Catalog {
    one: HashMap<String, String>,
    many: HashMap<String, Vec<String>>,
}

fn catalog(i: usize) -> &'static Catalog {
    static C: [OnceLock<Catalog>; LANGS.len()] = [const { OnceLock::new() }; LANGS.len()];
    C[i].get_or_init(|| parse(PO[i]))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}

/// The entries of a .po file; fuzzy and empty translations are left out (English shows).
fn parse(text: &str) -> Catalog {
    #[derive(Default)]
    struct Entry {
        fuzzy: bool,
        ctxt: bool,
        id: String,
        plural: Option<String>,
        strs: Vec<String>,
    }
    let mut cat = Catalog::default();
    let mut e = Entry::default();
    // which string continuation lines extend: 0 id, 1 plural, 2+k msgstr[k], 9 msgctxt
    let mut at = 0;
    let mut flush = |e: &mut Entry| {
        let e = std::mem::take(e);
        if e.fuzzy || e.ctxt || e.id.is_empty() || e.strs.iter().all(String::is_empty) {
            return;
        }
        if e.plural.is_some() {
            if e.strs.iter().all(|s| !s.is_empty()) {
                cat.many.insert(e.id, e.strs);
            }
        } else {
            cat.one.insert(e.id, e.strs.into_iter().next().unwrap_or_default());
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(c) = line.strip_prefix("#,") {
            flush(&mut e);
            e.fuzzy = c.contains("fuzzy");
        } else if line.starts_with('#') {
            continue;
        } else if let Some(s) = line.strip_prefix("msgctxt ") {
            if !e.strs.is_empty() {
                flush(&mut e);
            }
            e.ctxt = !unquote(s).is_empty();
            at = 9;
        } else if let Some(s) = line.strip_prefix("msgid_plural ") {
            e.plural = Some(unquote(s));
            at = 1;
        } else if let Some(s) = line.strip_prefix("msgid ") {
            // a "#," line already ended the entry before; else the msgid starts one
            if !e.strs.is_empty() {
                flush(&mut e);
            }
            e.id = unquote(s);
            at = 0;
        } else if let Some(s) = line.strip_prefix("msgstr[") {
            let (k, s) = s.split_once(']').unwrap_or(("0", ""));
            let k: usize = k.parse().unwrap_or(0);
            e.strs.resize(e.strs.len().max(k + 1), String::new());
            e.strs[k] = unquote(s);
            at = 2 + k;
        } else if let Some(s) = line.strip_prefix("msgstr ") {
            e.strs = vec![unquote(s)];
            at = 2;
        } else if line.starts_with('"') {
            let s = unquote(line);
            match at {
                0 => e.id.push_str(&s),
                1 => e.plural.get_or_insert_default().push_str(&s),
                9 => {}
                k => {
                    if let Some(x) = e.strs.get_mut(k - 2) {
                        x.push_str(&s);
                    }
                }
            }
        }
    }
    flush(&mut e);
    cat
}

/// `s` in the language in use.
pub fn tr(s: &'static str) -> &'static str {
    match current() {
        0 => s,
        i => catalog(i).one.get(s).map_or(s, String::as_str),
    }
}

/// `s` in the language in use with its placeholders filled.
pub fn trf(s: &'static str, args: &[&dyn Display]) -> String {
    fill(tr(s), args, None)
}

/// The singular or plural form for count `n`, `{n}` and the other placeholders filled.
pub fn trn(one: &'static str, many: &'static str, n: i64, args: &[&dyn Display]) -> String {
    let i = current();
    let t = (i != 0)
        .then(|| catalog(i).many.get(one))
        .flatten()
        .and_then(|forms| forms.get(plural(i, n)).or(forms.first()))
        .map_or(if n == 1 { one } else { many }, String::as_str);
    fill(t, args, Some(n))
}

fn fill(t: &str, args: &[&dyn Display], n: Option<i64>) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(t.len() + 16);
    let mut next = 0;
    let mut rest = t;
    while let Some(k) = rest.find(['{', '}']) {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        if rest.starts_with("{{") || rest.starts_with("}}") {
            out.push_str(&rest[..1]);
            rest = &rest[2..];
            continue;
        }
        let Some(end) = rest.find('}').filter(|_| rest.starts_with('{')) else {
            out.push_str(&rest[..1]);
            rest = &rest[1..];
            continue;
        };
        let key = &rest[1..end];
        rest = &rest[end + 1..];
        match key {
            "" => {
                if let Some(a) = args.get(next) {
                    let _ = write!(out, "{a}");
                }
                next += 1;
            }
            // with thousands separators, as the editor shows every count
            "n" if n.is_some() => out.push_str(&crate::fmt::num(n.unwrap())),
            k => match k.parse::<usize>().ok().and_then(|p| args.get(p)) {
                Some(a) => {
                    let _ = write!(out, "{a}");
                }
                None => {
                    out.push('{');
                    out.push_str(k);
                    out.push('}');
                }
            },
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        assert_eq!(fill("{} of {}", &[&1, &"two"], None), "1 of two");
        assert_eq!(fill("{1} before {0}", &[&"a", &"b"], None), "b before a");
        assert_eq!(fill("{n} items in {{box}}", &[], Some(1200)), "1,200 items in {box}");
        assert_eq!(fill("odd } and {x}", &[], None), "odd } and {x}");
    }

    #[test]
    fn po_entries() {
        let c = parse(concat!(
            "msgid \"\"\nmsgstr \"Plural-Forms: nplurals=2;\\n\"\n\n",
            "msgid \"Save\"\nmsgstr \"Enregistrer\"\n\n",
            "#, fuzzy\nmsgid \"Guess\"\nmsgstr \"Devinette\"\n\n",
            "msgid \"Empty\"\nmsgstr \"\"\n\n",
            "msgid \"A \\\"long\\\" \"\n\"line\"\nmsgstr \"\"\n\"Une ligne\"\n\n",
            "msgid \"{n} item\"\nmsgid_plural \"{n} items\"\nmsgstr[0] \"{n} objet\"\nmsgstr[1] \"{n} objets\"\n",
        ));
        assert_eq!(c.one.get("Save").map(String::as_str), Some("Enregistrer"));
        assert!(!c.one.contains_key("Guess") && !c.one.contains_key("Empty") && !c.one.contains_key(""));
        assert_eq!(c.one.get("A \"long\" line").map(String::as_str), Some("Une ligne"));
        assert_eq!(c.many["{n} item"], ["{n} objet", "{n} objets"]);
    }

    /// Every translation keeps its source's placeholders, so no value goes missing.
    #[test]
    fn catalogs_keep_placeholders() {
        fn marks(s: &str) -> Vec<String> {
            let mut v: Vec<String> = s.replace("{{", "").replace("}}", "").split('{').skip(1).filter_map(|p| p.split_once('}').map(|x| x.0.to_string())).collect();
            v.sort();
            v
        }
        for (i, (code, _)) in LANGS.iter().enumerate().skip(1) {
            let c = parse(PO[i]);
            for (id, t) in &c.one {
                assert_eq!(marks(id), marks(t), "{code}: {id:?} -> {t:?}");
            }
            for (id, forms) in &c.many {
                let want: Vec<String> = marks(id).into_iter().filter(|m| m != "n").collect();
                for t in forms {
                    let got: Vec<String> = marks(t).into_iter().filter(|m| m != "n").collect();
                    assert_eq!(want, got, "{code}: {id:?} -> {t:?}");
                }
            }
        }
    }

    #[test]
    fn plural_rules() {
        assert_eq!([0, 1, 2].map(|n| plural(index("fr"), n)), [0, 0, 1]);
        assert_eq!([0, 1, 2].map(|n| plural(index("de"), n)), [1, 0, 1]);
    }
}
