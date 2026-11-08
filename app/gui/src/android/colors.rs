//! Dynamic colour: the wallpaper's Material colours (Android 12 and newer) as the Material
//! library's light and dark schemes. MainActivity.colors() hands them as "name=aarrggbb"
//! lines with Android 14's names (system_primary_light...), worked out from the accent
//! palettes on Android 12 and 13. Older Android, or a role it does not give, keeps the
//! library's colours.

use crate::{AppWindow, MaterialPalette, MaterialScheme};
use slint::{Color, ComponentHandle};
use std::collections::HashMap;

fn parse(text: &str) -> HashMap<String, Color> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let argb = u32::from_str_radix(v.trim(), 16).ok()?;
            Some((k.trim().to_string(), Color::from_argb_encoded(argb)))
        })
        .collect()
}

/// The roles of one scheme (`sfx` "_light" or "_dark"; the fixed roles have none).
fn fill(m: &mut MaterialScheme, c: &HashMap<String, Color>, sfx: &str) -> usize {
    let mut n = 0;
    let mut get = |name: &str| {
        let v = c.get(&format!("system_{name}{sfx}")).or_else(|| c.get(&format!("system_{name}"))).copied();
        n += v.is_some() as usize;
        v
    };
    macro_rules! roles {
        ($($field:ident = $name:literal),* $(,)?) => {
            $( if let Some(v) = get($name) { m.$field = v; } )*
        };
    }
    roles! {
        primary = "primary", onPrimary = "on_primary", primaryContainer = "primary_container", onPrimaryContainer = "on_primary_container",
        secondary = "secondary", onSecondary = "on_secondary", secondaryContainer = "secondary_container", onSecondaryContainer = "on_secondary_container",
        tertiary = "tertiary", onTertiary = "on_tertiary", tertiaryContainer = "tertiary_container", onTertiaryContainer = "on_tertiary_container",
        error = "error", onError = "on_error", errorContainer = "error_container", onErrorContainer = "on_error_container",
        background = "background", onBackground = "on_background", surface = "surface", onSurface = "on_surface",
        surfaceVariant = "surface_variant", onSurfaceVariant = "on_surface_variant", outline = "outline", outlineVariant = "outline_variant",
        primaryFixed = "primary_fixed", onPrimaryFixed = "on_primary_fixed", primaryFixedDim = "primary_fixed_dim", onPrimaryFixedVariant = "on_primary_fixed_variant",
        secondaryFixed = "secondary_fixed", onSecondaryFixed = "on_secondary_fixed", secondaryFixedDim = "secondary_fixed_dim", onSecondaryFixedVariant = "on_secondary_fixed_variant",
        tertiaryFixed = "tertiary_fixed", onTertiaryFixed = "on_tertiary_fixed", tertiaryFixedDim = "tertiary_fixed_dim", onTertiaryFixedVariant = "on_tertiary_fixed_variant",
        surfaceDim = "surface_dim", surfaceBright = "surface_bright", surfaceContainerLowest = "surface_container_lowest", surfaceContainerLow = "surface_container_low",
        surfaceContainer = "surface_container", surfaceContainerHigh = "surface_container_high", surfaceContainerHighest = "surface_container_highest",
    }
    m.surfaceTint = m.primary;
    n
}

/// Sets the wallpaper's colours, if Android gives them.
pub fn apply(ui: &AppWindow) {
    let c = parse(&super::system_colors());
    if c.is_empty() {
        return;
    }
    let p = ui.global::<MaterialPalette>();
    let mut s = p.get_schemes();
    if fill(&mut s.light, &c, "_light") == 0 || fill(&mut s.dark, &c, "_dark") == 0 {
        return;
    }
    // the inverse roles (the snackbar), from the other scheme as Material builds them
    s.light.inversePrimary = s.dark.primary;
    s.dark.inversePrimary = s.light.primary;
    s.light.inverseSurface = s.dark.surfaceContainerHighest;
    s.light.inverseOnSurface = s.light.surfaceContainer;
    s.dark.inverseSurface = s.light.surfaceContainerHighest;
    s.dark.inverseOnSurface = s.dark.surfaceContainerHigh;
    p.set_schemes(s);
}
