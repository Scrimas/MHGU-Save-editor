//! The Advanced page: its models and the callbacks that edit it.

use super::*;

pub(super) fn fields_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let f = view(|v| v.field_filter.to_lowercase());
    let fields = &tables().fields;
    let abs = |fl: &mhgu_save::data::Field| if fl.block == "char1" { base + fl.rel } else { fl.abs };
    let rows: Vec<FieldRow> = fields
        .iter()
        .enumerate()
        .filter(|(_, fl)| {
            f.is_empty() || fl.label.to_lowercase().contains(&f) || fl.manager.to_lowercase().contains(&f) || format!("{:x}", abs(fl)).contains(f.trim_start_matches("0x"))
        })
        .map(|(i, fl)| FieldRow {
            index: i as i32,
            offset: format!("0x{:06X}", abs(fl)).into(),
            size: if fl.size >= 1024 { format!("{:.1} KB", fl.size as f32 / 1024.0) } else { format!("{} B", fl.size) }.into(),
            owner: fl.manager.clone().into(),
            label: fl.label.clone().into(),
            confidence: conf_of(&fl.confidence),
            changed: st.changed(abs(fl), fl.size.min(s.bytes().len() - abs(fl))),
        })
        .collect();
    api.set_fields(keep(api.get_fields(), rows));
    let sel = view(|v| v.field_sel);
    api.set_field_sel(sel);
    if let Some(fl) = (sel >= 0).then(|| fields.get(sel as usize)).flatten() {
        let a = abs(fl);
        let n = fl.size.min(512);
        let mut hex = String::new();
        for row in (0..n).step_by(16) {
            hex.push_str(&format!("{:06X}  ", a + row));
            for k in 0..16.min(n - row) {
                let o = a + row + k;
                let mark = if st.changed(o, 1) { '*' } else { ' ' };
                hex.push_str(&format!("{:02x}{mark}", s.u8(o)));
            }
            hex.push('\n');
        }
        if fl.size > n {
            let more = fl.size as i64 - n as i64;
            hex.push_str(&trn("… {} more byte", "… {} more bytes", more, &[&num(more)]));
            hex.push('\n');
        }
        api.set_hex(hex.into());
        let c = conf_of(&fl.confidence);
        let tag = match c {
            Confidence::Confirmed => tr("Confirmed"),
            Confidence::Derived => tr("Derived"),
            _ => tr("Unresolved"),
        };
        api.set_field_info(
            format!(
                "{}\n{} · {} · {} · {tag}",
                fl.label,
                fl.manager,
                trn("{} byte", "{} bytes", fl.size as i64, &[&num(fl.size as i64)]),
                if fl.block == "char1" { format!("base + 0x{:X}", fl.rel) } else { trf("block {}", &[&fl.block]) }
            )
            .into(),
        );
    } else {
        api.set_hex("".into());
        api.set_field_info("".into());
    }
}

pub(super) fn wire_advanced(ui: &AppWindow, st: &Shared) {
    // advanced
    on!(ui, st, on_filter_fields, |ui, s, f: SharedString| {
        view(|v| v.field_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_field, |ui, s, i: i32| {
        view(|v| v.field_sel = i);
        let _ = (&ui, &s);
    });
}
