//! The Guild Cards page: the cards kept, the inbox, the Hunters for Hire and the
//! blocked-user list (`mhgu_save::cards`). Every edit is Derived.

use super::*;
use crate::CardRow;
use chrono::Datelike;
use mhgu_save::cards::{self, List, Moved, Role};

/// Card type `k` as the list's *Change Card Type* names it (`GuildCardMsg` 211-234, 263).
pub fn card_type_name(k: i8) -> String {
    let names = [
        tr("Partner"), tr("Friend"), tr("Classmate"), tr("Teammate"), tr("Bandmate"), tr("Roommate"), tr("Family"), tr("Special"),
        tr("Co-worker"), tr("Colleague"), tr("VIP"), tr("Rival"), tr("Elite"), tr("Best Bud"), tr("Sensei"), tr("Online"), tr("Guild"),
        tr("Soldier"), tr("Scrivener"), tr("Team 1"), tr("Team 2"), tr("Team 3"), tr("Passerby"), tr("Sweetheart"),
    ];
    match k {
        cards::STREETPASS => tr("StreetPass").into(),
        0..=23 => names[k as usize].into(),
        _ => String::new(),
    }
}

/// A Guild hunter for hire, named by weapon type (`GuestHunterMsg` 29-42); the record
/// keeps only the first 11 characters.
fn hired_name(t: u8) -> String {
    match t {
        7 => tr("Hired Great Sword"),
        8 => tr("Hired Sword & Shield"),
        9 => tr("Hired Hammer"),
        10 => tr("Hired Lance"),
        11 => tr("Hired Heavy Bowgun"),
        13 => tr("Hired Light Bowgun"),
        14 => tr("Hired Long Sword"),
        15 => tr("Hired Switch Axe"),
        16 => tr("Hired Gunlance"),
        17 => tr("Hired Bow"),
        18 => tr("Hired Dual Blades"),
        19 => tr("Hired Hunting Horn"),
        20 => tr("Hired Insect Glaive"),
        21 => tr("Hired Charge Blade"),
        _ => return String::new(),
    }
    .into()
}

/// The weapon of a card or a hunter for hire: its name, else its class.
fn weapon_name(t: u8, id: u16) -> String {
    match Kind::from_code(t) {
        k @ Kind::Weapon(c) => piece(Owner::Hunter, k, id).map(|p| p.name.clone()).unwrap_or_else(|| weapon_classes().get(c as usize).copied().unwrap_or("?").to_string()),
        _ => String::new(),
    }
}

fn title_of(t: [u16; 3]) -> String {
    if t == [0, 0, 0] { String::new() } else { crate::targets::card_title(t) }
}

fn joined(v: Vec<String>) -> String {
    v.into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")
}

fn card_row(c: &cards::Card) -> CardRow {
    let (d, m, y) = c.date();
    let line1 = joined(vec![
        trf("HR {}", &[&c.hr]),
        c.kind().map(card_type_name).unwrap_or_default(),
        if c.list == List::Stored { trf("Unity {}", &[&num(c.unity())]) } else { String::new() },
        if c.list == List::Stored { trf("Updated {}", &[&fmt::date(d, m, y)]) } else { trf("Received {}", &[&fmt::date(d, m, y)]) },
    ]);
    let (t, id) = c.weapon();
    let line2 = joined(vec![
        title_of(c.title()),
        if c.prowler() { tr("Prowler").into() } else { weapon_name(t, id) },
        trn("{n} quest", "{n} quests", c.quests() as i64, &[]),
        fmt::playtime(c.playtime),
        if c.comment().is_empty() { String::new() } else { format!("“{}”", c.comment()) },
    ]);
    CardRow { slot: c.slot as i32, name: c.name.clone().into(), tag: SharedString::new(), line1: line1.into(), line2: line2.into(), greeting: c.greeting().into() }
}

pub(super) fn cards_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = s.base(st.slot);
    let stored = cards::cards(s, base, List::Stored);
    let inbox = cards::cards(s, base, List::Inbox);
    let blocked = cards::blacklist(s);
    let guests = cards::guests(s, base);
    api.set_card_tabs(strings([
        trf("Card List {} / {}", &[&stored.len(), &List::Stored.at().1]),
        trf("Guild Card Inbox {} / {}", &[&inbox.len(), &List::Inbox.at().1]),
        tr("Hunters for Hire").to_string(),
        trf("Blocked-user List {}", &[&blocked.len()]),
    ]));
    let tab = api.get_card_tab();
    let target = match tab {
        0 => Some(Target::Cards(false)),
        1 => Some(Target::Cards(true)),
        3 => Some(Target::Blacklist),
        _ => None,
    };
    api.set_was_cards(target.map(|t| st.was(t)).unwrap_or_default().into());
    api.set_was_cards_key(target.map(|t| t.key()).unwrap_or_default().into());
    let rows: Vec<CardRow> = match tab {
        0 => stored.iter().map(card_row).collect(),
        1 => inbox.iter().map(card_row).collect(),
        2 => guests
            .iter()
            .map(|g| {
                let tag = match g.role {
                    Role::Offered => tr("Offered"),
                    Role::Next => tr("Next offer"),
                    Role::Hired => tr("Hired"),
                };
                let (name, from) = if g.card { (g.name.clone(), tr("From a Guild Card")) } else { (hired_name(g.weapon.0), tr("Of the Guild")) };
                let line1 = joined(vec![trf("HR {}", &[&g.hr]), weapon_name(g.weapon.0, g.weapon.1), from.into()]);
                let line2 = if g.card { joined(vec![title_of(g.title), trf("Unity {}", &[&num(g.unity)])]) } else { String::new() };
                CardRow { slot: -1, name: name.into(), tag: tag.into(), line1: line1.into(), line2: line2.into(), greeting: SharedString::new() }
            })
            .collect(),
        _ => blocked
            .iter()
            .map(|b| {
                let id: String = b.id.iter().map(|x| format!("{x:02x}")).collect::<String>().trim_end_matches('0').to_string();
                CardRow { slot: b.slot as i32, name: b.name.clone().into(), line1: trf("Network ID {}", &[&id]).into(), ..Default::default() }
            })
            .collect(),
    };
    api.set_card_rows(model(rows));
    let summary: String = match tab {
        0 => tr("Cards received from other hunters. The game keeps them at the front of the list: removing one moves the later ones up, as its Delete does.").into(),
        1 => tr("Cards the Courier Service received. Move one to the Card List, as the Post Office does, or remove it.").into(),
        2 => match cards::hired_quests(s, base) {
            Some(n) => trn("The party is hired for {n} more quest. The game picks new offers after every quest, so this list is read-only.", "The party is hired for {n} more quests. The game picks new offers after every quest, so this list is read-only.", n as i64, &[]),
            None => tr("No one is hired. The game picks new offers after every quest, so this list is read-only.").into(),
        },
        _ => tr("Hunters you blocked online, shared by the three characters.").into(),
    };
    api.set_cards_summary(summary.into());
}

/// Every Derived: from code, the test save and a game-made list, not yet in game.
fn stage(ui: &AppWindow, s: &mut State, targets: Vec<Target>, title: String, f: impl FnOnce(&mut mhgu_save::Save, usize) -> bool) -> bool {
    if refused(ui, Conf::Derived) {
        return false;
    }
    let mut ok = false;
    let e = Edit { key: String::new(), title, detail: String::new(), note: String::new(), conf: Conf::Derived, targets };
    s.edit(e, |sv, base| {
        ok = f(sv, base);
        vec![]
    });
    ok
}

pub(super) fn wire_cards(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_card_tab, |ui, s, i: i32| {
        let _ = &s;
        ui.global::<Api>().set_card_tab(i.clamp(0, 3));
    });
    on!(ui, st, on_remove_card, |ui, s, slot: i32| {
        let Ok(slot) = usize::try_from(slot) else { return };
        let base = s.save().base(s.slot);
        match ui.global::<Api>().get_card_tab() {
            t @ (0 | 1) => {
                let l = if t == 0 { List::Stored } else { List::Inbox };
                let Some(c) = cards::cards(s.save(), base, l).into_iter().find(|c| c.slot == slot) else { return };
                stage(&ui, &mut s, vec![Target::Cards(t == 1)], trf("Remove the Guild Card of {}", &[&c.name]), |sv, base| cards::remove(sv, base, l, slot));
            }
            3 => {
                let Some(b) = cards::blacklist(s.save()).into_iter().find(|b| b.slot == slot) else { return };
                stage(&ui, &mut s, vec![Target::Blacklist], trf("Unblock {}", &[&b.name]), |sv, _| cards::unblock(sv, slot));
            }
            _ => {}
        }
    });
    on!(ui, st, on_move_card, |ui, s, slot: i32| {
        let Ok(slot) = usize::try_from(slot) else { return };
        let base = s.save().base(s.slot);
        let Some(c) = cards::cards(s.save(), base, List::Inbox).into_iter().find(|c| c.slot == slot) else { return };
        let now = chrono::Local::now();
        let today = (now.day() as u8, now.month() as u8, now.year() as u16);
        let probe = cards::move_to_stored(&mut s.save().clone(), base, slot, today);
        if probe == Some(Moved::Full) {
            toast(&ui, tr("Not changed: the Card List holds 100 cards"), true);
            return;
        }
        let title = trf("Move the Guild Card of {} to the Card List", &[&c.name]);
        if stage(&ui, &mut s, vec![Target::Cards(false), Target::Cards(true)], title, |sv, base| cards::move_to_stored(sv, base, slot, today).is_some())
            && probe == Some(Moved::Kept)
        {
            toast(&ui, tr("The Card List already holds as recent a card of this hunter: only the inbox card goes"), false);
        }
    });
}
