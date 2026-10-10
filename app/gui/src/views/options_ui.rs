//! The Options page: the character's game options, its chat phrases and the title menu's
//! settings shared by the characters (`mhgu_save::options`). Confirmed, except the
//! Simplified Chinese text language (Derived).

use super::*;
use crate::{OptionRow, PhraseRow};
use mhgu_save::options::{self, Choices, OPTIONS};

/// An option's name as its window shows it (`StartMenuMsg`).
pub fn option_name(k: usize) -> String {
    match OPTIONS[k].id {
        "music" => tr("Music Volume"),
        "sfx" => tr("SFX Volume"),
        "hud" => tr("HUD"),
        "map" => tr("Map"),
        "camera-angle" => tr("Camera Angle"),
        "camera-controls" => tr("Camera Controls"),
        "camera-speed" => tr("Camera Speed"),
        "scope" => tr("Scope Controls"),
        "bowgun" => tr("Bowgun Controls"),
        "quick-aim" => tr("Quick Aim Controls"),
        "quick-aim-camera" => tr("Quick Aim Camera"),
        "reticle" => tr("Reticle Speed"),
        "orientation" => tr("Orientation"),
        "bow" => tr("Bow Controls"),
        "bow-cancel" => tr("Bow Aim Mode Cancel"),
        "melody" => tr("Melody Effect Display"),
        "target-controls" => tr("Target Cam Controls"),
        "target-behavior" => tr("Target Cam Behavior"),
        "terrain-camera" => tr("Terrain-savvy Camera"),
        "lens-flares" => tr("Lens Flares"),
        "control-type" => tr("Control Type"),
        "plus-button" => tr("+ Button Controls"),
        "lr-buttons" => tr("L/R Button Controls"),
        "r-stick" => tr("R Stick Controls"),
        "net-cards" => tr("Guild Cards/Palicoes"),
        "net-chat" => tr("Chat"),
        "net-invites" => tr("Invites"),
        _ => tr("Palico Chat"),
    }
    .into()
}

fn choice_names(c: Choices) -> Vec<String> {
    match c {
        Choices::OnOff => vec![tr("On").into(), tr("Off").into()],
        Choices::Volume => std::iter::once(tr("Off").to_string()).chain((1..=6).map(|v| v.to_string())).chain([tr("Max").to_string()]).collect(),
        Choices::Types(n) => (1..=n).map(|v| trf("Type {}", &[&v])).collect(),
        Choices::Flip => [tr("Normal"), tr("Flip Y"), tr("Flip X"), tr("Flip X+Y")].map(String::from).to_vec(),
        Choices::Speed => [tr("Slow"), tr("Default"), tr("Fast")].map(String::from).to_vec(),
        Choices::AutoManual => [tr("Auto"), tr("Manual")].map(String::from).to_vec(),
        Choices::ManualSemi => [tr("Manual"), tr("Semi-auto")].map(String::from).to_vec(),
    }
}

/// Choice `v` of an option, the number itself past its choices.
pub fn choice_name(c: Choices, v: u8) -> String {
    choice_names(c).get(v as usize).cloned().unwrap_or_else(|| v.to_string())
}

/// "Group 1" (`OnlineMsg` 30-32).
pub fn chat_group_name(g: usize) -> String {
    trf("Group {}", &[&(g + 1)])
}

/// When auto-shoutout `i` is said (`OnlineMsg` 35-43).
pub fn auto_trigger(i: usize) -> String {
    match i {
        0 => tr("When mounting a monster"),
        1 => tr("When setting a trap"),
        2 => tr("When setting bombs"),
        3 => tr("When mobility is affected"),
        4 => tr("When pinned down"),
        5 => tr("When health is restored"),
        6 => tr("When using Hunter Art 1"),
        7 => tr("When using Hunter Art 2"),
        _ => tr("When using Hunter Art 3"),
    }
    .into()
}

/// The text language as the title menu names it (`TitleMsg` 79-83); the Chinese ones in
/// their own script, as the 1.4 menu shows them in every language.
pub fn language_name(v: u8) -> String {
    match v {
        0 => tr("Console language").into(),
        1 => tr("English").into(),
        2 => tr("French").into(),
        3 => tr("Spanish").into(),
        4 => tr("German").into(),
        5 => tr("Italian").into(),
        7 => "繁體中文".into(),
        8 => "简体中文".into(),
        v => trf("Language {}", &[&v]),
    }
}

pub(super) fn options_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = s.base(st.slot);
    let tab = api.get_option_tab();
    api.set_option_tabs(strings([tr("Options"), tr("Shoutouts"), tr("Game Settings")].map(String::from)));
    let was = |t: Target| SharedString::from(st.was(t));
    // a value past the choices picks none
    let pick = |id: String, name: String, choices: Vec<String>, value: i32, t: Target| OptionRow {
        id: id.into(),
        name: name.into(),
        value: if (0..choices.len() as i32).contains(&value) { value } else { -1 },
        choices: strings(choices),
        was: was(t),
        ..Default::default()
    };
    let rows: Vec<OptionRow> = match tab {
        0 => (0..OPTIONS.len())
            .map(|k| {
                // the row's id is its value's key (Undo)
                let mut r = pick(Target::Opt(k).key(), option_name(k), choice_names(OPTIONS[k].choices), options::option(s, base, k) as i32, Target::Opt(k));
                r.head = match k {
                    0 | 8 | 16 => trf("Options · page {}", &[&(k / 8 + 1)]),
                    24 => tr("Multiplayer Settings").into(),
                    _ => String::new(),
                }
                .into();
                r
            })
            .collect(),
        2 => {
            let lang = s.u8(options::LANGUAGE);
            vec![
                OptionRow {
                    id: "brightness".into(),
                    name: tr("TV Brightness").into(),
                    value: s.u8(options::BRIGHTNESS) as i32,
                    number: true,
                    max: options::MAX_BRIGHTNESS as i32,
                    was: was(Target::Brightness),
                    ..Default::default()
                },
                pick("rumble".into(), tr("Rumble").into(), choice_names(Choices::OnOff), !options::rumble(s) as i32, Target::Rumble),
                pick(
                    "language".into(),
                    tr("Language").into(),
                    options::LANGUAGES.iter().map(|&v| language_name(v)).collect(),
                    options::LANGUAGES.iter().position(|&v| v == lang).map_or(-1, |i| i as i32),
                    Target::Language,
                ),
            ]
        }
        _ => vec![],
    };
    api.set_option_rows(model(rows));
    let g = (api.get_chat_shown().clamp(0, options::GROUPS as i32 - 1)) as usize;
    api.set_chat_groups(strings((0..options::GROUPS).map(chat_group_name)));
    api.set_chat_group(options::chat_group(s, base) as i32);
    api.set_was_chat_group(was(Target::ChatGroup));
    api.set_was_auto_on(was(Target::AutoOn));
    let mut phrases: Vec<PhraseRow> = (0..options::PER_GROUP)
        .map(|i| PhraseRow {
            index: i as i32,
            head: if i == 0 { tr("Shoutouts").into() } else { SharedString::new() },
            label: (i + 1).to_string().into(),
            text: options::phrase(s, base, false, g, i).into(),
            was: was(Target::Phrase(false, g, i)),
            ..Default::default()
        })
        .collect();
    phrases.extend((0..options::AUTO_PER_GROUP).map(|i| PhraseRow {
        auto: true,
        index: i as i32,
        head: if i == 0 { tr("Auto-Shoutouts").into() } else { SharedString::new() },
        label: auto_trigger(i).into(),
        text: options::phrase(s, base, true, g, i).into(),
        on: options::auto_on(s, base, g, i),
        was: was(Target::Phrase(true, g, i)),
    }));
    api.set_phrases(model(phrases));
    api.set_options_summary(
        match tab {
            0 => tr("The Start Menu's Options and Multiplayer Settings of this character."),
            1 => tr("Each chat group holds 24 shoutouts and 9 auto-shoutouts, said when their event happens while they are on. A phrase holds up to 26 characters."),
            _ => tr("The title screen's settings, shared by the three characters."),
        }
        .into(),
    );
}

fn stage(ui: &AppWindow, s: &mut State, conf: Conf, t: Target, f: impl FnOnce(&mut mhgu_save::Save, usize)) {
    if refused(ui, conf) {
        return;
    }
    let title = t.label(s.save(), s.slot);
    s.edit(Edit::one(t, title, conf), |sv, base| {
        f(sv, base);
        vec![]
    });
}

pub(super) fn wire_options(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_option_tab, |ui, s, i: i32| {
        let _ = &s;
        ui.global::<Api>().set_option_tab(i.clamp(0, 2));
    });
    on!(ui, st, on_set_option, |ui, s, id: SharedString, v: i32| {
        let Ok(v) = u8::try_from(v) else { return };
        match id.as_str() {
            "brightness" => stage(&ui, &mut s, Conf::Confirmed, Target::Brightness, |sv, _| options::set_brightness(sv, v)),
            "rumble" => stage(&ui, &mut s, Conf::Confirmed, Target::Rumble, |sv, _| options::set_rumble(sv, v == 0)),
            "language" => {
                let Some(&l) = options::LANGUAGES.get(v as usize) else { return };
                // Simplified Chinese is Derived (options::LANGUAGE)
                let conf = if l == 8 { Conf::Derived } else { Conf::Confirmed };
                stage(&ui, &mut s, conf, Target::Language, |sv, _| {
                    options::set_language(sv, l);
                });
            }
            id => {
                let Some(k) = id.strip_prefix("opt:").and_then(|k| OPTIONS.iter().position(|o| o.id == k)) else { return };
                stage(&ui, &mut s, Conf::Confirmed, Target::Opt(k), |sv, base| {
                    options::set_option(sv, base, k, v);
                });
            }
        }
    });
    on!(ui, st, on_show_chat_group, |ui, s, g: i32| {
        let _ = &s;
        ui.global::<Api>().set_chat_shown(g.clamp(0, options::GROUPS as i32 - 1));
    });
    on!(ui, st, on_set_chat_group, |ui, s, g: i32| {
        let Ok(g) = u16::try_from(g) else { return };
        stage(&ui, &mut s, Conf::Confirmed, Target::ChatGroup, |sv, base| options::set_chat_group(sv, base, g));
    });
    on!(ui, st, on_set_phrase, |ui, s, auto: bool, i: i32, text: SharedString| {
        let g = ui.global::<Api>().get_chat_shown().clamp(0, options::GROUPS as i32 - 1) as usize;
        let n = if auto { options::AUTO_PER_GROUP } else { options::PER_GROUP };
        let Some(i) = usize::try_from(i).ok().filter(|&i| i < n) else { return };
        stage(&ui, &mut s, Conf::Confirmed, Target::Phrase(auto, g, i), |sv, base| options::set_phrase(sv, base, auto, g, i, &text));
    });
    on!(ui, st, on_set_auto_on, |ui, s, i: i32, on: bool| {
        let g = ui.global::<Api>().get_chat_shown().clamp(0, options::GROUPS as i32 - 1) as usize;
        let Some(i) = usize::try_from(i).ok().filter(|&i| i < options::AUTO_PER_GROUP) else { return };
        stage(&ui, &mut s, Conf::Confirmed, Target::AutoOn, |sv, base| options::set_auto_on(sv, base, g, i, on));
    });
}
