//! Original bytes use upstream browser storage and hooks, including the render workers.

use crate::{
    backend::Backend,
    files::Files,
    store::{Originals, content_hash, hash_of_path, original_path, storage_key},
};
use lightcraft_engine::catalog::{PhotoId, Source};
use lightcraft_ui_egui::{LightcraftApp, c0_host::decode_file};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
struct Host {
    originals: Originals,
    backend: Backend,
    saved: Rc<RefCell<HashSet<String>>>,
}

thread_local! {static HOST:RefCell<Option<Host>>=const {RefCell::new(None)};}

pub(super) fn attach(app: &mut LightcraftApp, originals: Originals, backend: Option<Backend>, files: Files) {
    app.c0.storage_error = Some(Box::new(move || files.error()));
    let Some(backend) = backend else {
        HOST.with(|slot| slot.borrow_mut().take());
        return;
    };
    let source = originals.clone();
    app.c0.store_file = Some(Box::new(move |name, bytes| {
        let hash = content_hash(&bytes);
        source.insert(&hash, bytes.into());
        Ok(original_path(&hash, name))
    }));
    let source = originals.clone();
    app.session.media.file_bytes =
        Some(Arc::new(move |path| source.get(path).map(|b| b.to_vec()).ok_or_else(|| format!("{path}: original is still loading"))));
    HOST.with(|slot| *slot.borrow_mut() = Some(Host { originals, backend, saved: Rc::default() }));
}

pub(super) struct Work {
    host: Host,
    kind: Kind,
}
enum Kind {
    Store { hash: String, bytes: Arc<[u8]> },
    Read { hash: String },
}

pub(super) fn prepare(app: &LightcraftApp, method: &str, p: &mut Value) -> Result<Option<Work>, String> {
    let Some(host) = HOST.with(|slot| slot.borrow().clone()) else {
        return Ok(None);
    };
    match method {
        "c0.openFile" => {
            let (_, bytes) = decode_file(p)?;
            let hash = content_hash(&bytes);
            if host.saved.borrow().contains(&hash) {
                host.originals.insert(&hash, bytes.into());
                return Ok(None);
            }
            Ok(Some(Work { host, kind: Kind::Store { hash, bytes: bytes.into() } }))
        }
        "c0.documentBytes" => {
            if p.get("format").is_some_and(|v| !matches!(v.as_str(), Some("jpeg" | "png" | "tiff"))) {
                return Ok(None);
            }
            let id = match p.get("photo") {
                Some(v) => v.as_u64().map(PhotoId),
                None => app.session.active(),
            };
            let Some(id) = id else {
                return Ok(None);
            };
            let Some(photo) = app.session.catalog.photo(id).filter(|p| p.in_library()) else {
                return Ok(None);
            };
            let Source::File { path } = &photo.source else {
                return Ok(None);
            };
            if host.originals.contains(path) {
                return Ok(None);
            }
            let Some(hash) = hash_of_path(path).map(str::to_owned) else {
                return Ok(None);
            };
            // The requested active photo stays the target even if selection changes during I/O.
            if let Some(p) = p.as_object_mut() {
                p.insert("photo".into(), json!(id.0));
            }
            Ok(Some(Work { host, kind: Kind::Read { hash } }))
        }
        _ => Ok(None),
    }
}

pub(super) async fn perform(work: Work) -> Result<(), String> {
    let (hash, bytes) = match work.kind {
        Kind::Store { hash, bytes } => {
            work.host.backend.write(&storage_key(&hash), &bytes).await.map_err(|e| format!("saving the original: {e}"))?;
            (hash, bytes)
        }
        Kind::Read { hash } => {
            let bytes =
                work.host.backend.read(&storage_key(&hash)).await?.ok_or_else(|| format!("original {hash} is missing from browser storage"))?;
            (hash, Arc::from(bytes))
        }
    };
    work.host.originals.insert(&hash, bytes);
    work.host.saved.borrow_mut().insert(hash);
    Ok(())
}
