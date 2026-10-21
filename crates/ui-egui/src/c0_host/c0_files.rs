//! Byte import uses the engine's probe, loader and library.import; exports use its one encoder.

use crate::LightcraftApp;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use lightcraft_catalog::PhotoId;
use lightcraft_engine::{
    Session,
    export::{self, ExportOptions},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const MIB: usize = 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 72 * MIB;
pub const MAX_REQUEST_BYTES: usize = 100 * MIB;
pub fn encode_bytes(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}
const MAX_LIBRARY_BYTES: usize = MAX_FILE_BYTES * 4;
const MAX_NAME_BYTES: usize = 255;

type Originals = Arc<Mutex<BTreeMap<String, Arc<[u8]>>>>;

#[derive(Default)]
pub(super) struct Files {
    originals: Originals,
    installed: bool,
}

pub fn decode_file(p: &Value) -> Result<(String, Vec<u8>), String> {
    let name = p.get("name").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).ok_or("name is required")?;
    if name.len() > MAX_NAME_BYTES || name.contains(['/', '\\', '\0']) || matches!(name, "." | "..") {
        return Err(format!("name must be a file basename of at most {MAX_NAME_BYTES} bytes"));
    }
    if !lightcraft_engine::import::is_supported(std::path::Path::new(name)) {
        return Err("unsupported photo extension".into());
    }
    let text = p.get("bytes").and_then(Value::as_str).ok_or("bytes must be standard padded base64 text")?;
    if text.len() > MAX_FILE_BYTES.div_ceil(3) * 4 {
        return Err(format!("file exceeds {} MiB", MAX_FILE_BYTES / MIB));
    }
    let bytes = STANDARD.decode(text).map_err(|e| format!("invalid base64: {e}"))?;
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
        return Err(format!("file is empty or exceeds {} MiB", MAX_FILE_BYTES / MIB));
    }
    lightcraft_engine::files::probe_bytes(name, &bytes).map_err(|e| format!("{name}: {e}"))?;
    Ok((name.into(), bytes))
}

fn ready(app: &mut LightcraftApp) -> Result<(), String> {
    if let Some(error) = app.c0.storage_error.as_ref().and_then(|status| status()) {
        return Err(format!("browser changes are unsaved: {error}; retry after storage recovers"));
    }
    crate::panels::settings::can_switch_library(app)?;
    if app.export.is_some() || app.merge.busy() {
        return Err("wait for export/merge to finish".into());
    }
    // The same save-failure guard used before leaving a library. Never discard unlogged ops.
    if app.session.persist().is_err() {
        // As at quit, a fresh closing snapshot may recover a failed journal append.
        app.session.close_library().map_err(|e| e.to_string())?;
    }
    if let Some((ops, error)) = app.session.unsaved() {
        return Err(format!("{ops} changes are unsaved: {error}"));
    }
    Ok(())
}

fn install(app: &mut LightcraftApp) {
    if app.c0.files.installed {
        return;
    }
    app.c0.files.installed = true;
    let files = app.c0.files.originals.clone();
    let fallback = app.session.media.file_loader.clone();
    app.session.media.file_loader = Some(Arc::new(move |path, edge| {
        let bytes = files.lock().unwrap_or_else(|e| e.into_inner()).get(path).cloned();
        match bytes {
            Some(bytes) => lightcraft_engine::files::load_bytes(&bytes, edge),
            None => fallback.as_ref().ok_or_else(|| format!("{path}: original unavailable"))?(path, edge),
        }
    }));
    let files = app.c0.files.originals.clone();
    let fallback = app.session.media.file_probe.clone();
    app.session.media.file_probe = Some(Arc::new(move |path| {
        let bytes = files.lock().unwrap_or_else(|e| e.into_inner()).get(path).cloned();
        match bytes {
            Some(bytes) => lightcraft_engine::files::probe_bytes(path, &bytes),
            None => fallback.as_ref().ok_or_else(|| format!("{path}: original unavailable"))?(path),
        }
    }));
    let files = app.c0.files.originals.clone();
    let fallback = app.session.media.file_bytes.clone();
    app.session.media.file_bytes = Some(Arc::new(move |path| {
        let bytes = files.lock().unwrap_or_else(|e| e.into_inner()).get(path).cloned();
        match bytes {
            Some(bytes) => Ok(bytes.to_vec()),
            None => fallback.as_ref().ok_or_else(|| format!("{path}: original unavailable"))?(path),
        }
    }));
    let files = app.c0.files.originals.clone();
    let fallback = app.session.media.preview_loader.clone();
    app.session.media.preview_loader = Some(Arc::new(move |path, edge| {
        let bytes = files.lock().unwrap_or_else(|e| e.into_inner()).get(path).cloned();
        match bytes {
            Some(bytes) => lightcraft_engine::files::embedded_preview_srgb(&bytes, edge),
            None => fallback.as_ref().and_then(|load| load(path, edge)),
        }
    }));
}

pub(super) fn open(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    let (name, bytes) = decode_file(p)?;
    ready(app)?;
    if app.session.library.as_ref().is_some_and(|library| library.on_disk) {
        return Err("host bytes require a guest memory document; use c0.newDocument first, or import file paths into a disk library".into());
    }
    if let Some(store) = app.c0.store_file.as_mut() {
        let path = store(&name, bytes)?;
        return app.run("library.import", json!({"paths":[path],"mode":"add"}));
    }
    install(app);
    let path = format!("web/{}/{name}", lightcraft_preview::hash_bytes(&bytes));
    let inserted = {
        let mut files = app.c0.files.originals.lock().unwrap_or_else(|e| e.into_inner());
        let total = files.values().fold(0usize, |n, b| n.saturating_add(b.len()));
        if !files.contains_key(&path) && total.saturating_add(bytes.len()) > MAX_LIBRARY_BYTES {
            return Err(format!("host originals exceed the {} MiB session budget; start a new document first", MAX_LIBRARY_BYTES / MIB));
        }
        files.insert(path.clone(), bytes.into()).is_none()
    };
    let result = app.run("library.import", json!({"paths":[path],"mode":"add"}));
    let referenced =
        app.session.catalog.photos().any(|photo| matches!(&photo.source, lightcraft_catalog::Source::File { path: original } if original == &path));
    // A command can apply its op and then report a failed save. Keep that photo's original.
    // Conversely, an import rejected as a duplicate must not consume the session byte budget.
    if inserted && !referenced {
        app.c0.files.originals.lock().unwrap_or_else(|e| e.into_inner()).remove(&path);
    }
    result
}

pub(super) fn new_document(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    if let Some(template) = p.get("template")
        && !matches!(template.as_str(), Some("" | "blank"))
    {
        return Err("template must be blank".into());
    }
    ready(app)?;
    // Preserve saved panel/settings preferences while clearing serde-skipped photo/gesture state.
    let mut ui: crate::UiState = serde_json::to_value(&app.ui).and_then(serde_json::from_value).map_err(|e| format!("reset view state: {e}"))?;
    ui = ui.sanitized();
    ui.view = crate::state::ViewMode::PhotoGrid;
    ui.search.clear();
    ui.renaming_component = None;
    app.session.close_library().map_err(|e| e.to_string())?;
    cancel(app);
    let mut session = Session::new();
    session.segmenter.background = true;
    std::mem::swap(&mut session.clock, &mut app.session.clock);
    session.media.preview_capacity = app.session.media.preview_capacity;
    session.media.file_loader = app.session.media.file_loader.clone();
    session.media.file_probe = app.session.media.file_probe.clone();
    session.media.file_bytes = app.session.media.file_bytes.clone();
    session.media.preview_loader = app.session.media.preview_loader.clone();
    app.session = session;
    app.c0.files.originals.lock().unwrap_or_else(|e| e.into_inner()).clear();
    app.caches = Default::default();
    app.renderer.forget_all();
    app.services.backup_library = None;
    app.memory_applied = None;
    app.gpu_applied = None;
    app.display_applied = None;
    app.ui = ui;
    app.gesture = None;
    app.brush_live = None;
    app.hover_preview = None;
    app.loupe_shown = None;
    app.lightroom_last = None;
    app.lightroom_result = None;
    app.shown_library = None;
    app.library_problem = None;
    Ok(json!({"photoCount":0,"template":"blank"}))
}

pub(super) fn cancel(app: &mut LightcraftApp) -> Value {
    let native = !app.pending_picks.is_empty();
    let browser = app.c0.cancel_picker.as_mut().is_some_and(|p| p());
    let cancelled = native || browser;
    // Dropping each receiver abandons its result; no cancelled picker imports a file later.
    app.pending_picks.clear();
    json!({"cancelled":cancelled})
}

pub(super) fn document_bytes(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    let format = match p.get("format") {
        None => "jpeg",
        Some(Value::String(f)) if matches!(f.as_str(), "jpeg" | "png" | "tiff" | "xmp") => f,
        _ => return Err("format must be jpeg, png, tiff or xmp".into()),
    };
    let id = match p.get("photo") {
        Some(value) => PhotoId(value.as_u64().ok_or("photo must be an id")?),
        None => app.session.active().ok_or("no active photo")?,
    };
    let photo = app.session.catalog.photo(id).filter(|p| p.in_library()).ok_or("no such photo")?.clone();
    let bytes = if format == "xmp" {
        lightcraft_engine::sidecar::sidecar_packet(&photo, &app.session.catalog).into_bytes()
    } else {
        let options = ExportOptions::from_params(&json!({"format":format})).map_err(|e| e.to_string())?;
        export::export_photo(&mut app.session, id, &options, 1)?.bytes
    };
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("document exceeds {} MiB", MAX_FILE_BYTES / MIB));
    }
    let stem = std::path::Path::new(&photo.file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("photo");
    Ok(json!({"name":format!("{stem}.{format}"),"format":format,"bytes":STANDARD.encode(bytes),
        "revision":app.session.catalog.revision,"photo":id.0}))
}
