//! Colours from matugen (Linux). The template in app/packaging/matugen, registered in
//! matugen's config.toml, writes a dark and a light Material palette to `matugen.json` in
//! the config folder each time the wallpaper changes; the editor maps them onto its own
//! tokens (Tok) and the widgets' accent. Any tool that writes the same JSON works too: two
//! objects, "dark" and "light", each with the [`ROLES`] as "#rrggbb" (or "#rrggbbaa").

use crate::Scheme;
use slint::Color;
use std::path::{Path, PathBuf};

pub const TEMPLATE: &str = include_str!("../../packaging/matugen/mhgu-save-editor.json");
/// Material roles read from each palette.
pub const ROLES: [&str; 9] = [
    "primary",
    "on_surface",
    "on_surface_variant",
    "surface_container_lowest",
    "surface_container_low",
    "surface_container",
    "surface_container_high",
    "surface_container_highest",
    "error",
];

pub fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mhgu-save-editor"))
}
/// The colours matugen writes.
pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("matugen.json"))
}
/// Where Settings saves the template for matugen to read.
pub fn template_path() -> Option<PathBuf> {
    dir().map(|d| d.join("matugen-template.json"))
}

/// `p` with `~` for the home folder, as matugen's config.toml writes paths.
pub fn tilde(p: &Path) -> String {
    match dirs::home_dir().and_then(|h| p.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(rel) => format!("~/{}", rel.display()),
        None => p.display().to_string(),
    }
}

/// The lines to add to matugen's config.toml.
pub fn snippet() -> String {
    let t = |p: Option<PathBuf>| tilde(&p.unwrap_or_default());
    format!("[templates.mhgu-save-editor]\ninput_path = '{}'\noutput_path = '{}'", t(template_path()), t(path()))
}

/// The dark and light schemes of a matugen.json, and the accent for the widgets.
pub struct Loaded {
    pub dark: Scheme,
    pub light: Scheme,
    pub accent: Color,
}

pub fn load(p: &Path) -> Result<Loaded, String> {
    let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
    parse(&text)
}

fn parse(text: &str) -> Result<Loaded, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let roles = |mode: &str| -> Result<[Color; 9], String> {
        let o = v.get(mode).ok_or(format!("no \"{mode}\" palette"))?;
        let mut out = [Color::default(); 9];
        for (k, role) in ROLES.iter().enumerate() {
            let s = o.get(role).and_then(|x| x.as_str()).ok_or(format!("{mode}.{role} missing"))?;
            out[k] = hex(s).ok_or(format!("{mode}.{role}: {s:?} is not #rrggbb"))?;
        }
        Ok(out)
    };
    let (d, l) = (roles("dark")?, roles("light")?);
    // Fluent keeps its own lightness and takes hue and chroma from the accent; the light
    // primary has the stronger chroma of the two
    Ok(Loaded { dark: scheme(&d, true), light: scheme(&l, false), accent: l[0] })
}

fn hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    if !matches!(h.len(), 6 | 8) || !h.is_ascii() {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    let a = if h.len() == 8 { b(6)? } else { 255 };
    Some(Color::from_argb_u8(a, b(0)?, b(2)?, b(4)?))
}

/// Material roles to the editor's tokens. Surfaces step up in dark and down to white
/// cards in light, as the built-in colours do; the hairlines, hover and text-box tints
/// are on_surface at the built-in alphas. Text itself stays the widgets' (Scheme).
fn scheme(c: &[Color; 9], dark: bool) -> Scheme {
    let [primary, text, muted, lowest, low, mid, high, highest, error] = *c;
    let a = |d: u8, l: u8| text.with_alpha(if dark { d } else { l } as f32 / 255.0);
    Scheme {
        page: if dark { low } else { mid },
        side: if dark { mid } else { high },
        card: if dark { high } else { lowest },
        raised: if dark { highest } else { lowest },
        muted,
        faint: a(0x5e, 0x5e),
        line: a(0x17, 0x14),
        track: a(0x1f, 0x1a),
        hover: a(0x0d, 0x0a),
        accent_text: primary,
        bad: error,
        input: if dark { a(0x0f, 0) } else { lowest.with_alpha(0xb3 as f32 / 255.0) },
        input_active: lowest,
        input_border: a(0x14, 0x0f),
        input_edge: a(0x8a, 0x73),
    }
}

/// Sets the accent the Fluent widgets derive theirs from; None gives back the desktop's.
/// Linux only, as matugen (i-slint-core is a Linux dependency).
#[cfg(not(target_os = "linux"))]
pub fn set_accent(_: &slint::Window, _: Option<Color>) {}
#[cfg(target_os = "linux")]
pub fn set_accent(w: &slint::Window, c: Option<Color>) {
    thread_local! {
        static DESKTOP: std::cell::Cell<Option<Color>> = const { std::cell::Cell::new(None) };
    }
    let ctx = i_slint_core::window::WindowInner::from_pub(w).context();
    let desktop = DESKTOP.with(|d| *d.get().get_or_insert_with(|| ctx.accent_color()));
    ctx.set_accent_color(c.unwrap_or(desktop));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_fills_every_role() {
        // a template run with every placeholder replaced by one colour parses
        let filled = fill(TEMPLATE, "#336699");
        let t = parse(&filled).unwrap();
        assert_eq!(t.dark.muted, Color::from_rgb_u8(0x33, 0x66, 0x99));
        assert_eq!(t.light.card, Color::from_rgb_u8(0x33, 0x66, 0x99));
        assert!(parse(r#"{"dark":{}}"#).err().unwrap().contains("dark.primary"));
        assert_eq!(hex("#11223380"), Some(Color::from_argb_u8(0x80, 0x11, 0x22, 0x33)));
        assert_eq!(hex("112233"), None);
        assert_eq!(hex("#12345"), None);
    }

    fn fill(t: &str, c: &str) -> String {
        let mut out = String::new();
        let mut rest = t;
        while let Some(i) = rest.find("{{") {
            out.push_str(&rest[..i]);
            out.push_str(c);
            rest = &rest[rest[i..].find("}}").unwrap() + i + 2..];
        }
        out + rest
    }
}
