use crate::LightcraftApp;
use serde_json::{Value, json};

pub(super) fn inspect(app: &LightcraftApp) -> Value {
    let c = app.c0.chrome;
    json!({"title_bar":c.title_bar,"top_bar":c.top_bar,"start_screen":c.start_screen,
        "community_links":c.community_links,"experimental_notice":c.experimental_notice,
        "tool_strip":c.tool_strip,"bottom_bar":c.bottom_bar,
        "left_panel":app.ui.left_panel,"filmstrip":app.ui.filmstrip,"histogram":app.ui.histogram,
        "filter_bar":app.ui.filter_bar,"navigator":app.ui.navigator})
}

pub(super) fn set(app: &mut LightcraftApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    let mut report = json!({"applied":[],"unknown":[],"invalid":[]});
    if let Some(tokens) = p.get("themeTokens") {
        crate::theme::host::set(ctx, tokens, &mut report);
    }
    if let Some(panels) = p.get("panels") {
        if let Some(entries) = panels.as_object() {
            for (key, value) in entries {
                if matches!(key.as_str(), "title_bar" | "top_bar") {
                    let field = format!("panels.{key}");
                    if let Some(on) = value.as_bool() {
                        app.c0.chrome.title_bar = on;
                        app.c0.chrome.top_bar = on;
                        record(&mut report, "applied", field);
                    } else {
                        record(&mut report, "invalid", field);
                    }
                    continue;
                }
                let slot = match key.as_str() {
                    "start_screen" => Some(&mut app.c0.chrome.start_screen),
                    "community_links" => Some(&mut app.c0.chrome.community_links),
                    "experimental_notice" => Some(&mut app.c0.chrome.experimental_notice),
                    "tool_strip" => Some(&mut app.c0.chrome.tool_strip),
                    "bottom_bar" => Some(&mut app.c0.chrome.bottom_bar),
                    "left_panel" => Some(&mut app.ui.left_panel),
                    "filmstrip" => Some(&mut app.ui.filmstrip),
                    "histogram" => Some(&mut app.ui.histogram),
                    "filter_bar" => Some(&mut app.ui.filter_bar),
                    "navigator" => Some(&mut app.ui.navigator),
                    _ => None,
                };
                let field = format!("panels.{key}");
                match (slot, value.as_bool()) {
                    (None, _) => record(&mut report, "unknown", field),
                    (Some(_), None) => record(&mut report, "invalid", field),
                    (Some(slot), Some(value)) => {
                        *slot = value;
                        record(&mut report, "applied", field);
                    }
                }
            }
        } else {
            record(&mut report, "invalid", "panels".into());
        }
    }
    // Docking derives visibility from UiState on the next pass.
    ctx.request_repaint();
    Ok(report)
}

pub(crate) fn record(report: &mut Value, kind: &str, key: String) {
    if let Some(values) = report.get_mut(kind).and_then(Value::as_array_mut) {
        values.push(json!(key));
    }
}
