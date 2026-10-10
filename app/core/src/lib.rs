//! Monster Hunter Generations Ultimate (Switch) save: read, edit, write.
//!
//! Everything here follows the notes in ../../docs; offsets are relative to a character
//! base unless named `abs`. The editor only writes fields documented as CONFIRMED or
//! DERIVED.

pub mod arena;
pub mod cards;
pub mod character;
pub mod data;
pub mod equipment;
pub mod guildcard;
pub mod items;
pub mod mhxx;
pub mod monsters;
pub mod options;
pub mod otomo;
pub mod palico;
pub mod progress;
pub mod save;
pub mod sets;
pub mod slots;
pub mod smithy;
pub mod store;
pub mod transfer;
pub mod unlocks;

pub use save::{Error, Save};

#[cfg(test)]
mod real_save {
    //! Checks against a real save, ignored by default: set MHGU_TEST_SAVE to a copy of
    //! `0/system` and run `cargo test -- --ignored`.
    use super::*;

    fn path() -> std::ffi::OsString {
        std::env::var_os("MHGU_TEST_SAVE").expect("set MHGU_TEST_SAVE to a copy of 0/system")
    }

    fn load() -> Save {
        Save::from_bytes(std::fs::read(path()).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn documented_values() {
        let mut s = load();
        assert_eq!(s.base(0), save::SLOT1_BASE);
        assert!(s.slot_used(0));
        let base = s.base(0);
        // the computed HR equals the copies the game wrote
        let st = character::get(&s, base);
        assert_eq!(st.hr, s.u16(base + character::HDR_HR));
        assert_eq!(st.hr, s.u16(base + character::CARD_HR));
        // item box: every used slot has a count of 1-99
        let box_ = items::all(&s, base, items::Store::Box);
        let used = box_.iter().filter(|x| !x.is_empty()).count();
        assert!(used > 100, "{used} used slots");
        assert!(box_.iter().filter(|x| !x.is_empty()).all(|x| (1..=99).contains(&x.count)));
        // monster 48 (Arzuros) and 121 (Rustrazor) have size records (docs/02)
        for i in [48, 121] {
            let r = monsters::get(&s, base, i);
            assert!(r.min > 0 && r.min <= r.max, "{i}: {r:?}");
        }
        // round trip: rewriting every slot with what was read changes nothing
        items::set_all(&mut s, base, items::Store::Box, &box_);
        for k in 0..items::LOADOUT_N {
            let l = items::loadout(&s, base, k);
            items::set_loadout(&mut s, base, k, &l);
        }
        // the Guild Card log rebuilt from the records equals the card the game wrote
        for i in 1..=monsters::N {
            if let Some(p) = monsters::meta(i).card_pos {
                assert_eq!(monsters::card_entry(&s, base, i), s.get(base + monsters::CARD_LOG + 8 * p, 8), "card entry of {i}");
            }
        }
        for i in 1..=monsters::N {
            let r = monsters::get(&s, base, i);
            // the editor artefacts on 106-112 / folded variants (docs: sizes never read) stay
            let before = s.get(base + monsters::SIZES + 4 * i, 4).to_vec();
            monsters::set(&mut s, base, i, r);
            assert_eq!(before, s.get(base + monsters::SIZES + 4 * i, 4));
        }
        for i in 0..palico::LIST_N {
            let p = palico::get(&s, base, i);
            palico::set(&mut s, base, i, &p);
        }
        assert!(s.diff().is_empty(), "round trip changed {:?}", &s.diff()[..s.diff().len().min(8)]);
    }

    /// New entries look like the game's own; worn and My Set pieces are found; the free
    /// slot is empty and unreferenced.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn equipment_box() {
        use equipment::{Entry, Kind, Owner, Use};
        let s = load();
        let base = s.base(0);
        for i in 0..Owner::Hunter.len() {
            let e = equipment::get(&s, base, Owner::Hunter, i);
            match e.kind() {
                Kind::Talisman => assert_eq!(e.talisman().unwrap().tier, equipment::talisman_tier(e.id()), "talisman {i}"),
                // a fresh entry of the same piece differs only in level and decorations
                k @ Kind::Weapon(_) => {
                    let mut n = Entry::new(k, e.id());
                    n.set_level(e.level());
                    for (d, &item) in e.decos().iter().enumerate() {
                        n.set_deco(d, item);
                    }
                    assert_eq!(n, e, "weapon {i}");
                }
                _ => {}
            }
        }
        for k in 0..7 {
            let i = s.u16(base + equipment::WORN + 2 * k);
            if i != 0xFFFF {
                assert!(equipment::uses(&s, base, i as usize).contains(&Use::Worn), "worn {i}");
                assert!(!equipment::get(&s, base, Owner::Hunter, i as usize).is_empty());
            }
        }
        let f = equipment::free_slot(&s, base, Owner::Hunter).unwrap();
        assert!(equipment::get(&s, base, Owner::Hunter, f).is_empty() && equipment::uses(&s, base, f).is_empty());
    }

    /// The loaded set registered again is the set the game wrote; rewriting every set's
    /// pieces and arts as read changes nothing (the decorations are the box entries').
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn sets_match_the_game() {
        let mut s = load();
        let base = s.base(0);
        let worn: Vec<u16> = (0..sets::PIECES).map(|p| s.u16(base + equipment::WORN + 2 * p)).collect();
        // (none after the editor changed the worn gear)
        if let Some(k) = (0..sets::MY_SETS_N).find(|&k| sets::my_set(&s, base, k).gear[..] == worn[..]) {
            let m = sets::my_set(&s, base, k);
            assert!(m.arts == sets::arts(&s, base) && m.pigment == sets::pigment(&s, base), "set {} is the one worn", k + 1);
            sets::save_current(&mut s, base, k);
        }
        for k in 0..sets::MY_SETS_N {
            let m = sets::my_set(&s, base, k);
            for (p, &g) in m.gear.iter().enumerate() {
                sets::set_my_set_piece(&mut s, base, k, p, (g != sets::NO_BOX).then_some(g as usize));
            }
            sets::set_my_set_arts(&mut s, base, k, m.arts);
            if !m.used() {
                sets::clear_my_set(&mut s, base, k);
            }
        }
        for k in 0..sets::PALICO_SETS_N {
            if !sets::palico_set(&s, base, k).used() {
                sets::clear_palico_set(&mut s, base, k);
            }
        }
        let a = sets::arts(&s, base);
        sets::set_arts(&mut s, base, a);
        assert!(s.diff().is_empty(), "changed {:?}", &s.diff()[..s.diff().len().min(8)]);
    }

    /// Every bit of every unlock map of a played character is one of the map's entries,
    /// and writing each entry back as read changes nothing.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn unlock_maps_match_the_game() {
        let mut s = load();
        let base = s.base(0);
        for (m, mp) in unlocks::maps().iter().enumerate() {
            let known: Vec<usize> = unlocks::entries(m).iter().map(|e| e.bit).collect();
            let set: Vec<usize> = (0..8 * mp.bytes).filter(|&b| unlocks::on(&s, base, m, b)).collect();
            assert!(set.iter().all(|b| known.contains(b)), "{}: {set:?}", mp.id);
            for b in known {
                let v = unlocks::on(&s, base, m, b);
                if unlocks::refusal(&s, base, m, b, v).is_none() {
                    assert!(unlocks::set(&mut s, base, m, b, v), "{} {b}", mp.id);
                }
            }
        }
        for k in 0..unlocks::PETS {
            let (n, c) = (unlocks::pet_name(&s, base, k), unlocks::pet_costume(&s, base, k));
            unlocks::set_pet_name(&mut s, base, k, &n);
            assert!(unlocks::set_pet_costume(&mut s, base, k, c), "pet {k} wears {c}");
        }
        assert!(s.diff().is_empty(), "changed {:?}", &s.diff()[..s.diff().len().min(8)]);
    }

    /// Edit a copy of the whole save folder and write it: all four files get the body,
    /// each keeps its header, and nothing but the edited bytes moves.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn write_copy_of_real_save() {
        let p = path();
        let src = std::path::Path::new(&p).parent().unwrap().parent().unwrap();
        let dir = std::env::temp_dir().join(format!("mhgu-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for c in ["0", "1"] {
            std::fs::create_dir_all(dir.join(c)).unwrap();
            for f in store::FILES {
                std::fs::copy(src.join(c).join(f), dir.join(c).join(f)).unwrap();
            }
        }
        let before: Vec<Vec<u8>> = ["0/system", "0/system_backup", "1/system", "1/system_backup"].iter().map(|f| std::fs::read(dir.join(f)).unwrap()).collect();
        let (mut s, mut loc, _) = store::open(&dir.join("0/system")).unwrap();
        let base = s.base(0);
        let mut r = monsters::get(&s, base, 48);
        r.hunts = 1234;
        monsters::set(&mut s, base, 48, r);
        let edited: Vec<usize> = s.diff().iter().map(|d| d.0).collect();
        assert!(!edited.is_empty());
        store::write_all(&mut s, &mut loc).unwrap();
        for (k, f) in ["0/system", "0/system_backup", "1/system", "1/system_backup"].iter().enumerate() {
            let after = std::fs::read(dir.join(f)).unwrap();
            assert_eq!(after[..store::HEADER], before[k][..store::HEADER], "{f}: header kept");
            assert_eq!(monsters::get(&Save::from_bytes(after.clone()).unwrap(), base, 48).hunts, 1234, "{f}");
            let moved: Vec<usize> = (0..after.len()).filter(|&i| after[i] != before[0][i] && i >= store::HEADER).collect();
            assert!(moved.iter().all(|i| edited.contains(i)), "{f}: unexpected bytes changed");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn quest_logic_runs() {
        let mut s = load();
        let c = progress::Char::new(&mut s, 0);
        let mut locked = 0;
        for q in progress::Char::real_quests(true) {
            if let progress::Lock::Locked(n) = c.lock(q.id) {
                assert!(n.iter().all(|alt| alt.iter().all(|m| !m.starts_with('?'))), "{}: {n:?}", q.id);
                locked += 1;
            }
        }
        eprintln!("locked quests: {locked}");
        // like tools/request_offer.py: only offers not accepted yet
        for r in data::tables().offers.iter().filter(|o| !c.flag(o.accept_flag)) {
            for m in c.offer_missing(r.index) {
                assert!(!m.starts_with('?'), "offer {}: {m}", r.index);
            }
        }
    }

    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn stored_cards_and_guests() {
        use cards::{List, Role};
        let mut s = load();
        let base = s.base(0);
        let c = cards::cards(&s, base, List::Stored);
        let seen = |c: &[cards::Card]| c.iter().map(|c| (c.name.clone(), c.hr, c.unity(), c.kind())).collect::<Vec<_>>();
        // the trailer repeats the card's HR and name
        assert!(c.iter().all(|c| c.data[0x16..0x18] == c.hr.to_le_bytes() && c.data[..2 * c.name.encode_utf16().count()] == c.name.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()[..]));
        eprintln!("{:?} · {:?}", seen(&c), c.iter().map(|c| (c.title(), c.weapon(), c.quests(), c.date())).collect::<Vec<_>>());
        assert_eq!(c.len(), 2);
        assert!(cards::cards(&s, base, List::Inbox).is_empty());
        let g = cards::guests(&s, base);
        eprintln!("{:?}", g.iter().map(|g| (g.role, g.name.clone(), g.hr, g.weapon, g.card)).collect::<Vec<_>>());
        assert_eq!(g.iter().filter(|g| g.role == Role::Hired).count(), 4);
        assert!(g.iter().filter(|g| g.card).all(|g| c.iter().any(|c| c.name == g.name)));
        let kept = c[1].clone();
        assert!(cards::remove(&mut s, base, List::Stored, 0));
        let after = cards::cards(&s, base, List::Stored);
        assert_eq!(seen(&after), seen(std::slice::from_ref(&kept)));
        assert_eq!((after[0].slot, &after[0].data), (0, &kept.data));
    }
}
