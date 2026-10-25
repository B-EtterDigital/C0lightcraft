//! `c0.selection` and `c0.place`: what the C0VIBE AI actions read from C0light and put back.
//!
//! Not upstream's: part of the C0VIBE host shell (`c0/lightcraft-host/README.md`). A placement goes
//! through the app's own byte import (`c0.openFile`'s path) and the engine's `stack.group`, so the
//! library, history and saving behave as for a user import, and the whole placement is ONE undo step.
//!
//! | Method | Params | Result |
//! |---|---|---|
//! | `c0.selection` | `{photo?}` | `{kind:"photo", photoId, name, width, height, sourceWidth, sourceHeight, png, mask?}` |
//! | `c0.place` | `{png\|jpeg, mode:"new-version", name?, source?}` | `{placed:"photo", mode, photoId, name, stackedWith, bytes}` |

use base64::{Engine as _, engine::general_purpose::STANDARD};
use lightcraft_catalog::PhotoId;
use lightcraft_codecs::{EncodeImage, EncodeMeta, Samples, encode_png};
use lightcraft_pipeline::{MaskView, Overlay};
use serde_json::{Value, json};

use crate::LightcraftApp;

/// The biggest picture a request may carry, decoded.
pub const MAX_PICTURE_BYTES: usize = 40 * 1024 * 1024;
/// The longest side of the picture `c0.selection` sends.
pub const MAX_SELECTION_EDGE: usize = 2048;
const MAX_NAME_BYTES: usize = 255;
const DEFAULT_STEM: &str = "ai-result";


// ---------- c0.selection ----------

fn photo_param(p: &Value, key: &str) -> Result<Option<PhotoId>, String> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(|n| Some(PhotoId(n))).ok_or_else(|| format!("`{key}` must be a photo id")),
    }
}

pub fn selection(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    if app.ui.view != crate::state::ViewMode::Detail {
        return Err("open a photo in Develop first".into());
    }
    let id = app.session.active().ok_or("open a photo in Develop first")?;
    if let Some(asked) = photo_param(p, "photo")?
        && asked != id
    {
        return Err("that photo is not the one open in Develop".into());
    }
    let photo = app.session.catalog.photo(id).filter(|p| p.in_library()).ok_or("no such photo")?.clone();
    let render = |app: &mut LightcraftApp, overlay: Overlay| -> Result<lightcraft_raster::Rgba8, String> {
        let job = app.session.render_job(id, MAX_SELECTION_EDGE, MAX_SELECTION_EDGE, false, true).ok_or("the photo cannot be rendered")?;
        let job = if overlay == Overlay::None { job } else { job.with_overlay(overlay) };
        Ok(job.run().rendered?.image)
    };
    let image = render(app, Overlay::None)?;
    let (width, height) = (image.width, image.height);
    let png = encode_png(&EncodeImage::rgba8(&image), &EncodeMeta::default()).map_err(|e| format!("encoding the picture: {e}"))?;
    let mut out = json!({
        "kind": "photo", "photoId": id.0, "name": photo.file_name, "width": width, "height": height,
        "sourceWidth": photo.width, "sourceHeight": photo.height, "png": STANDARD.encode(png),
    });
    let mask = app.session.active_mask.filter(|m| photo.develop.masks.iter().any(|k| k.id == *m && !k.components.is_empty()));
    if let Some(mask) = mask.and_then(|m| u16::try_from(m).ok()) {
        let view = render(app, Overlay::Mask { id: mask, view: MaskView::WhiteOnBlack, color: [255, 0, 0], opacity: 100 })?;
        let grey: Vec<u8> = view.data.iter().map(|px| px[0]).collect();
        let bytes = encode_png(&EncodeImage::new(view.width as u32, view.height as u32, 1, Samples::U8(&grey)), &EncodeMeta::default())
            .map_err(|e| format!("encoding the mask: {e}"))?;
        out["mask"] = json!(STANDARD.encode(bytes));
    }
    Ok(out)
}

// ---------- c0.place ----------

/// The file the page stores for a placed picture: `(file name, base64 text, extension)`. Shared with
/// the page bridge, which stores the original bytes before the placement runs.
pub fn place_file(p: &Value) -> Result<(String, String), String> {
    let kinds: Vec<&str> = ["png", "jpeg"].into_iter().filter(|k| p.get(*k).is_some_and(|v| !v.is_null())).collect();
    let kind = match kinds.as_slice() {
        [one] => *one,
        [] => return Err("nothing to place: give `png` or `jpeg`".into()),
        _ => return Err("place one picture at a time: give only one of `png` or `jpeg`".into()),
    };
    let text = p[kind].as_str().ok_or_else(|| format!("`{kind}` must be standard padded base64 text"))?;
    if text.len() > MAX_PICTURE_BYTES.div_ceil(3) * 4 {
        return Err(format!("the picture exceeds {} MB", MAX_PICTURE_BYTES / (1024 * 1024)));
    }
    let stem = match p.get("name") {
        None | Some(Value::Null) => DEFAULT_STEM.to_owned(),
        Some(Value::String(n)) => {
            let n = n.trim();
            if n.is_empty() || n.len() > MAX_NAME_BYTES || n.contains(['/', '\\', '\0']) || matches!(n, "." | "..") {
                return Err(format!("`name` must be a file name of at most {MAX_NAME_BYTES} bytes, without folders"));
            }
            std::path::Path::new(n).file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or(DEFAULT_STEM).to_owned()
        }
        Some(_) => return Err("`name` must be text".into()),
    };
    let ext = if kind == "png" { "png" } else { "jpg" };
    Ok((format!("{stem}.{ext}"), text.to_owned()))
}

pub fn place(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    match p.get("mode") {
        Some(Value::String(m)) if m == "new-version" => {}
        _ => return Err("`mode` must be \"new-version\"".into()),
    }
    let (name, text) = place_file(p)?;
    let source = match photo_param(p, "source")? {
        Some(id) => Some(id),
        None => app.session.active(),
    };
    if let Some(id) = source
        && !app.session.catalog.photo(id).is_some_and(|p| p.in_library())
    {
        return Err("no such source photo in the library".into());
    }
    let file = json!({"name": name, "bytes": text});
    // Decode and probe first: a bad picture fails before anything changes.
    let (_, bytes) = super::decode_file(&file)?;
    if bytes.len() > MAX_PICTURE_BYTES {
        return Err(format!("the picture exceeds {} MB", MAX_PICTURE_BYTES / (1024 * 1024)));
    }
    drop(bytes);
    let before: std::collections::BTreeSet<u64> = app.session.catalog.photos().map(|p| p.id.0).collect();
    let commits = app.session.commits();
    let undo_back = |app: &mut LightcraftApp| {
        for _ in 0..app.session.commits().wrapping_sub(commits) {
            if app.session.undo_step().is_err() {
                break;
            }
        }
    };
    let done = (|| -> Result<PhotoId, String> {
        super::c0_files::open(app, &file)?;
        let new = app
            .session
            .catalog
            .photos()
            .map(|p| p.id)
            .find(|id| !before.contains(&id.0))
            .ok_or("that picture is already in the library")?;
        if let Some(src) = source {
            app.run("stack.group", json!({"ids": [src.0, new.0], "top": src.0, "collapsed": false}))?;
        }
        Ok(new)
    })();
    let n = app.session.commits().wrapping_sub(commits) as usize;
    match done {
        Ok(new) => {
            app.session.merge_undo(n, "Place AI picture");
            Ok(json!({"placed": "photo", "mode": "new-version", "photoId": new.0, "name": name, "stackedWith": source.map(|s| s.0),
                "bytes": bytes_len(&text)}))
        }
        Err(e) => {
            undo_back(app);
            Err(e)
        }
    }
}

fn bytes_len(text: &str) -> usize {
    text.len() / 4 * 3 - text.bytes().rev().take(2).filter(|b| *b == b'=').count()
}
