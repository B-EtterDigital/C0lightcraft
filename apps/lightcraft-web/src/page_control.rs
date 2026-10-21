//! Opt-in control envelope around LightCraft's existing browser command queue.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use serde::Deserialize;
use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(target_arch = "wasm32")]
mod files;
#[cfg(target_arch = "wasm32")]
mod io;
#[cfg(target_arch = "wasm32")]
pub use bridge::{attach, run, runner};

pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_INLINE_PASSES: usize = 40;
pub const MAX_CLICK_COUNT: u64 = 256;
pub const MAX_DRAG_STEPS: u64 = 32;
pub const POLL_MS: i32 = 4;
pub const PUMP_MS: f64 = 16.0;
pub const COMMAND: &str = "__page.control";
pub const PICKER_COMMAND: &str = "__page.picker";
pub const READY_COMMAND: &str = "__page.ready";
pub const MAX_PENDING_IO: usize = 4;

#[derive(Deserialize)]
struct Envelope<'a> {
    method: String,
    #[serde(borrow)]
    params: Option<&'a serde_json::value::RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LargeFile {
    name: String,
    bytes: String,
}

pub fn parse(text: &str) -> Result<(String, Value), String> {
    if text.len() > lightcraft_ui_egui::c0_host::MAX_REQUEST_BYTES {
        return Err("request exceeds the host request limit".into());
    }
    // Borrow raw params first: large file bytes are allowed, a large Value tree is not.
    let envelope: Envelope<'_> = serde_json::from_str(text).map_err(|e| format!("request is not JSON: {e}"))?;
    let method = envelope.method.as_str();
    if method.is_empty() {
        return Err("request has no method".into());
    }
    if method != "c0.openFile" && text.len() > MAX_REQUEST_BYTES {
        return Err("request is larger than 1 MiB".into());
    }
    let raw = envelope.params.map_or("{}", |p| p.get());
    let params: Value = if text.len() > MAX_REQUEST_BYTES {
        let file: LargeFile = serde_json::from_str(raw).map_err(|e| format!("invalid file request: {e}"))?;
        json!({"name":file.name,"bytes":file.bytes})
    } else {
        serde_json::from_str(raw).map_err(|e| format!("invalid params: {e}"))?
    };
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err("params must be an object".into());
    }
    let limit = match method {
        "ui.click" | "ui.clickWidget" => Some(("count", MAX_CLICK_COUNT)),
        "ui.drag" | "ui.dragWidget" => Some(("steps", MAX_DRAG_STEPS)),
        _ => None,
    };
    if let Some((key, max)) = limit
        && params.get(key).is_some_and(|v| v.as_u64().is_none_or(|n| n > max))
    {
        return Err(format!("{key} must be a nonnegative integer at most {max}"));
    }
    if let Some(events) = params.get("events").and_then(Value::as_array)
        && events.len() > MAX_CLICK_COUNT as usize
    {
        return Err("too many pointer events".into());
    }
    if method.starts_with("ui.") {
        validate_coordinates(&params)?;
        if let Some(events) = params.get("events").and_then(Value::as_array) {
            for event in events {
                validate_coordinates(event)?;
            }
        }
    }
    Ok((method.into(), params))
}

fn validate_coordinates(p: &Value) -> Result<(), String> {
    for key in ["x", "y", "toX", "toY", "dx", "dy", "fx", "fy", "width", "height"] {
        if let Some(value) = p.get(key)
            && value.as_f64().is_none_or(|n| !(n as f32).is_finite())
        {
            return Err(format!("{key} must be a finite coordinate"));
        }
    }
    Ok(())
}

pub fn reads_only(method: &str) -> bool {
    matches!(method, "c0.inspect" | "c0.documentBytes" | "ui.inspect" | "ui.widgets" | "ui.menu.list" | "ui.menu.tree" | "engine.commands")
}

pub fn inline_passes(method: &str) -> usize {
    if matches!(method, "ui.key" | "ui.text" | "ui.click" | "ui.move" | "ui.drag" | "ui.pointer" | "ui.scroll" | "ui.clickWidget" | "ui.dragWidget") {
        MAX_INLINE_PASSES
    } else {
        0
    }
}

pub fn pump_due(wants_repaint: bool, now: f64, last: f64) -> bool {
    wants_repaint && now - last >= PUMP_MS
}
