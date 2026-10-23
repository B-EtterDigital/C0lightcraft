//! C0light's guest contract. Defaults leave upstream chrome and autosave semantics intact.

mod c0_chrome;
mod c0_files;
pub(crate) mod c0_panels;
pub(crate) mod c0_strip;

pub use c0_chrome::{Chrome, about_links, community_url, contributor_link, external_link};
pub use c0_files::{MAX_FILE_BYTES, MAX_REQUEST_BYTES, decode_file, encode_bytes};
pub use c0_strip::Modes;

use lightcraft_catalog::{AlbumId, PhotoId};
use serde_json::{Value, json};

use crate::{LightcraftApp, control::Outcome};

#[derive(Default)]
pub struct Host {
    pub chrome: Chrome,
    pub modes: Modes,
    pub picker_pending: Option<Box<dyn Fn() -> bool>>,
    pub cancel_picker: Option<Box<dyn FnMut() -> bool>>,
    pub store_file: Option<Box<dyn FnMut(&str, Vec<u8>) -> Result<String, String>>>,
    pub storage_error: Option<Box<dyn Fn() -> Option<String>>>,
    files: c0_files::Files,
}

pub fn handle(app: &mut LightcraftApp, ctx: &egui::Context, method: &str, params: &Value) -> Outcome {
    let result = if !params.is_object() && !params.is_null() {
        Err("params must be an object".into())
    } else {
        match method {
            "c0.inspect" => inspect(app, ctx),
            "c0.goTo" => go_to(app, params),
            "c0.documentBytes" => c0_files::document_bytes(app, params),
            "c0.openFile" => c0_files::open(app, params),
            "c0.newDocument" => c0_files::new_document(app, params),
            "c0.cancelFileDialog" => Ok(c0_files::cancel(app)),
            "c0.set" => c0_panels::set(app, ctx, params),
            _ => Err(format!("unknown host method {method:?}")),
        }
    };
    Outcome::Done(match result {
        Ok(result) => json!({"ok": true, "result": result}),
        Err(error) => json!({"ok": false, "error": error}),
    })
}

fn inspect(app: &mut LightcraftApp, ctx: &egui::Context) -> Result<Value, String> {
    let unsaved = app
        .session
        .unsaved()
        .map(|(ops, error)| json!({"ops":ops,"error":error}))
        .or_else(|| app.c0.storage_error.as_ref().and_then(|status| status()).map(|error| json!({"ops":0,"error":error,"storage":true})));
    let active = app
        .session
        .active()
        .and_then(|id| app.session.catalog.photo(id))
        .map(|p| json!({"id":p.id.0,"name":p.file_name,"rating":p.rating,"flag":p.flag}));
    let collections: Vec<_> = app
        .session
        .catalog
        .albums()
        .filter(|a| !a.folder)
        .map(|a| json!({"id":a.id.0,"name":a.name,"count":app.session.catalog.album_count(a.id)}))
        .collect();
    Ok(json!({
        "revision": app.session.catalog.revision, "dirty": unsaved.is_some(),
        "canUndo": !app.session.undo.is_empty(),
        "canRedo": !app.session.redo.is_empty(),
        "panes": c0_panels::inspect(app),
        "theme": crate::theme::current(ctx).unwrap_or_default().id(),
        "photoCount": app.session.catalog.photos().filter(|p| p.in_library()).count(),
        "selectedCount": app.session.selection.ids.len(), "activePhoto": active,
        "view": app.ui.view, "collections":collections,
        "importing": app.import.is_some() || app.scan.is_some() || crate::lightroom_import::is_running(app),
        "exporting": app.export.is_some(), "unsaved":unsaved,
        "fileDialog": !app.pending_picks.is_empty() || app.c0.picker_pending.as_ref().is_some_and(|p|p()),
    }))
}

fn go_to(app: &mut LightcraftApp, p: &Value) -> Result<Value, String> {
    // Validate the entire navigation before changing source, selection or view.
    let id = |key: &str| -> Result<Option<u64>, String> {
        p.get(key).map(|v| v.as_u64().ok_or_else(|| format!("{key} must be a photo/collection id"))).transpose()
    };
    let photo = id("photo")?.map(PhotoId);
    let collection = id("collection")?.map(AlbumId);
    if let Some(id) = photo
        && !app.session.catalog.photo(id).is_some_and(|p| p.in_library())
    {
        return Err("no such photo in the library".into());
    }
    if let Some(id) = collection
        && app.session.catalog.album(id).is_none_or(|a| a.folder)
    {
        return Err("no such collection".into());
    }
    if let (Some(a), Some(id)) = (collection, photo)
        && !app.session.catalog.album_photos(a).contains(&id)
    {
        return Err("photo is not in the collection".into());
    }
    let view = p
        .get("view")
        .map(|v| {
            let v = v.as_str().ok_or("view must be a view name")?;
            serde_json::from_value::<crate::state::ViewMode>(json!(if v == "grid" { "photoGrid" } else { v })).map_err(|_| "unknown view".to_owned())
        })
        .transpose()?;
    if let Some(id) = collection {
        app.run("library.source", json!({"kind":"album","id":id.0}))?;
    } else if photo.is_some() {
        app.run("library.source", json!({"kind":"all"}))?;
    }
    if let Some(id) = photo {
        app.run("library.clearFilter", json!({}))?;
        app.run("library.select", json!({"ids":[id.0],"active":id.0}))?;
    }
    if let Some(view) = view {
        app.ui.view = view;
    }
    Ok(json!({"photo":app.session.active().map(|p|p.0),"collection":collection.map(|a|a.0),"view":app.ui.view}))
}
