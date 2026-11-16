//! The phone interface's pagers (ui/mobile/gestures.slint). While a swipe brings a
//! neighbouring tab in beside the one shown, that tab's rows are built here into the page's
//! Peek global, by the code that builds them for the page itself. The desktop interface has
//! no pagers and is built without this.

use super::*;

pub(super) fn wire_peek(ui: &AppWindow, st: &Shared) {
    // a page's `build(tab)`: the preview of that tab
    macro_rules! peek {
        ($global:ty, $build:path) => {{
            let (w, st) = (ui.as_weak(), st.clone());
            ui.global::<$global>().on_build(move |tab| {
                if let Some(ui) = w.upgrade() {
                    let st = st.borrow();
                    if st.doc.is_some() {
                        $build(&ui, &st, tab.max(0) as usize);
                    }
                }
            });
        }};
    }
    peek!(crate::CollectionsPeek, collections::peek_collections);
    peek!(crate::ItemsPeek, items_page::peek_items);
    peek!(crate::EquipmentPeek, equipment_page::peek_equipment);
    peek!(crate::QuestsPeek, quests::peek_quests);
    peek!(crate::PalicoesPeek, palicoes::peek_palicoes);
    peek!(crate::OptionsPeek, options_ui::peek_options);
    peek!(crate::CardsPeek, cards_ui::peek_cards);

    // a destination coming in beside the page shown: its models, as when it is the page
    let (w, st2) = (ui.as_weak(), st.clone());
    ui.global::<crate::Gest>().on_build_page(move |page| {
        if let Some(ui) = w.upgrade() {
            let st = st2.borrow();
            if st.doc.as_ref().is_some_and(|d| d.save.slot_used(st.slot)) {
                build_page(&ui, &st, &page);
            }
        }
    });
}
