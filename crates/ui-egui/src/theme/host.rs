//! Host colour overrides use their own egui id and survive theme-family changes.

use super::Tokens;
use egui::{Color32, Context, Id};
use serde_json::Value;
use std::collections::BTreeMap;

fn store() -> Id {
    Id::new("c0light-theme-tokens")
}

pub fn set(ctx: &Context, value: &Value, report: &mut Value) {
    let Some(entries) = value.as_object() else {
        crate::c0_host::c0_panels::record(report, "invalid", "themeTokens".into());
        return;
    };
    let mut colors = ctx.data(|d| d.get_temp::<BTreeMap<String, Color32>>(store())).unwrap_or_default();
    for (key, value) in entries {
        let field = format!("themeTokens.{key}");
        let kind = if !assign(&mut Tokens::default(), key, Color32::TRANSPARENT) {
            "unknown"
        } else if value.is_null() {
            colors.remove(key);
            "applied"
        } else if let Some(color) = colour(value) {
            colors.insert(key.clone(), color);
            "applied"
        } else {
            "invalid"
        };
        crate::c0_host::c0_panels::record(report, kind, field);
    }
    ctx.data_mut(|d| d.insert_temp(store(), colors));
    super::apply_kind(ctx, super::current(ctx).unwrap_or_default());
}

pub(super) fn resolve(ctx: &Context, mut tokens: Tokens) -> Tokens {
    if let Some(colors) = ctx.data(|d| d.get_temp::<BTreeMap<String, Color32>>(store())) {
        for (key, color) in colors {
            assign(&mut tokens, &key, color);
        }
    }
    tokens
}

fn colour(value: &Value) -> Option<Color32> {
    let hex = value.as_str()?.strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at| hex.get(at..at + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    Some(Color32::from_rgba_unmultiplied(byte(0)?, byte(2)?, byte(4)?, if hex.len() == 8 { byte(6)? } else { 255 }))
}

fn assign(t: &mut Tokens, key: &str, color: Color32) -> bool {
    macro_rules! colors {
        ($($field:ident),* $(,)?) => {
            match key { $(stringify!($field) => { t.$field = color; true },)* _ => false }
        };
    }
    colors!(
        chrome,
        canvas,
        grid_bg,
        cell,
        cell_selected,
        divider,
        inset,
        field,
        field_border,
        button,
        button_border,
        hover,
        pressed,
        tool_active,
        text,
        text_label,
        text_dim,
        text_disabled,
        icon,
        track,
        thumb,
        thumb_hover,
        accent,
        star,
        pick,
        reject,
        caution,
        mask_overlay,
        skeleton
    )
}
