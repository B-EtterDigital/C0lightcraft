use base64::{Engine as _, engine::general_purpose::STANDARD};
use lightcraft_engine::Session;
use lightcraft_ui_egui::{
    LightcraftApp, Services,
    c0_host::Chrome,
    control::{self, ControlRequest, Outcome},
    headless::Headless,
    theme::{self, ThemeKind, Tokens},
};
use serde_json::{Value, json};
use std::time::Duration;

fn make_app() -> LightcraftApp {
    LightcraftApp::new(Session::new(), Services::default())
}

fn request(app: &mut LightcraftApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
    let (req, _) = ControlRequest::new(method, params);
    match control::handle(app, ctx, &req) {
        Outcome::Done(value) => value,
        Outcome::Screenshot { .. } => panic!("unexpected screenshot"),
        Outcome::Gesture { .. } => panic!("unexpected gesture"),
    }
}

fn png() -> Vec<u8> {
    let image = lightcraft_raster::Rgba8::from_bytes(3, 2, &[40, 90, 160, 255].repeat(6)).unwrap();
    lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(&image), &Default::default()).unwrap()
}

fn open(app: &mut LightcraftApp, ctx: &egui::Context) -> Value {
    request(app, ctx, "c0.openFile", json!({"name":"original.png","bytes":STANDARD.encode(png())}))
}

#[test]
fn defaults_keep_every_upstream_chrome_surface() {
    let c = Chrome::default();
    assert!(c.title_bar && c.top_bar && c.start_screen && c.community_links && c.experimental_notice && c.tool_strip && c.bottom_bar);
    assert_eq!(Chrome::for_page(false), c);
    let c = Chrome::for_page(true);
    assert!(!c.title_bar && !c.top_bar && !c.start_screen && !c.community_links && !c.experimental_notice);
    assert!(c.tool_strip && c.bottom_bar);
}

#[test]
fn every_host_method_returns_an_envelope_on_an_empty_library() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    for (method, params, ok) in [
        ("c0.inspect", json!({}), true),
        ("c0.goTo", json!({"view":"grid"}), true),
        ("c0.documentBytes", json!({}), false),
        ("c0.openFile", json!({}), false),
        ("c0.newDocument", json!({}), true),
        ("c0.cancelFileDialog", json!({}), true),
        ("c0.set", json!({}), true),
        ("c0.unknown", json!({}), false),
    ] {
        let reply = request(&mut app, &ctx, method, params);
        assert_eq!(reply["ok"], ok, "{method}: {reply}");
    }
    assert_eq!(request(&mut app, &ctx, "c0.inspect", json!([]))["ok"], false);
}

#[test]
fn inspect_mirrors_real_library_selection_history_and_tasks() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let reply = request(&mut app, &ctx, "c0.inspect", json!({}));
    let state = &reply["result"];
    for key in [
        "revision",
        "dirty",
        "canUndo",
        "canRedo",
        "panes",
        "theme",
        "photoCount",
        "selectedCount",
        "activePhoto",
        "view",
        "collections",
        "importing",
        "exporting",
        "unsaved",
    ] {
        assert!(state.get(key).is_some(), "{key}");
    }
    assert_eq!(state["photoCount"], 0);
    assert_eq!(state["dirty"], false);
    assert_eq!(open(&mut app, &ctx)["ok"], true);
    let id = app.session.catalog.photos().next().unwrap().id.0;
    assert_eq!(request(&mut app, &ctx, "c0.goTo", json!({"photo":id,"view":"detail"}))["ok"], true);
    app.run("photo.rate", json!({"rating":3})).unwrap();
    let state = request(&mut app, &ctx, "c0.inspect", json!({}));
    assert_eq!(state["result"]["photoCount"], 1);
    assert_eq!(state["result"]["selectedCount"], 1);
    assert_eq!(state["result"]["activePhoto"]["id"], id);
    assert_eq!(state["result"]["activePhoto"]["rating"], 3);
    assert_eq!(state["result"]["canUndo"], true);
    assert_eq!(state["result"]["view"], "detail");
}

#[test]
fn navigation_validates_before_changing_any_state() {
    let mut app = LightcraftApp::new(Session::with_demo(), Services::default());
    let ctx = egui::Context::default();
    let id = app.session.catalog.photos().next().unwrap().id.0;
    let album = app.session.catalog.albums().find(|a| !a.folder).unwrap().id.0;
    let before = request(&mut app, &ctx, "c0.inspect", json!({}));
    for params in [
        json!({"photo":id,"view":"wrong"}),
        json!({"collection":u64::MAX,"view":"detail"}),
        json!({"photo":-1}),
        json!({"photo":u64::MAX}),
        json!({"collection":"bad"}),
    ] {
        assert_eq!(request(&mut app, &ctx, "c0.goTo", params)["ok"], false);
        assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({})), before);
    }
    assert_eq!(request(&mut app, &ctx, "c0.goTo", json!({"collection":album}))["ok"], true);
    for view in ["grid", "photoGrid", "squareGrid", "detail", "compare", "survey", "reference", "people"] {
        assert_eq!(request(&mut app, &ctx, "c0.goTo", json!({"photo":id,"view":view}))["ok"], true, "{view}");
    }
}

#[test]
fn all_document_formats_export_read_only_and_raster_formats_reopen() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    assert_eq!(open(&mut app, &ctx)["ok"], true);
    let id = app.session.catalog.photos().next().unwrap().id.0;
    request(&mut app, &ctx, "c0.goTo", json!({"photo":id}));
    app.run("develop.set", json!({"control":"light.exposure","value":0.5})).unwrap();
    let before = request(&mut app, &ctx, "c0.inspect", json!({}));
    let settings = app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).unwrap().develop.clone();
    for format in ["jpeg", "png", "tiff", "xmp"] {
        let reply = request(&mut app, &ctx, "c0.documentBytes", json!({"format":format,"photo":id}));
        assert_eq!(reply["ok"], true, "{reply}");
        let bytes = STANDARD.decode(reply["result"]["bytes"].as_str().unwrap()).unwrap();
        assert!(!bytes.is_empty());
        if format == "xmp" {
            assert!(String::from_utf8(bytes).unwrap().contains("lc:settings"));
        } else {
            let mut fresh = make_app();
            let reply = request(&mut fresh, &ctx, "c0.openFile", json!({"name":format!("again.{format}"),"bytes":reply["result"]["bytes"]}));
            assert_eq!(reply["ok"], true, "{format}: {reply}");
            assert_eq!(fresh.session.catalog.photos().filter(|p| p.in_library()).count(), 1);
        }
        assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({})), before);
        assert_eq!(app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).unwrap().develop, settings);
    }
    assert_eq!(request(&mut app, &ctx, "c0.documentBytes", json!({"format":"avif"}))["ok"], false);
    assert_eq!(request(&mut app, &ctx, "c0.documentBytes", json!({"photo":u64::MAX}))["ok"], false);
}

#[test]
fn invalid_files_and_templates_preserve_the_library() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    open(&mut app, &ctx);
    let before = request(&mut app, &ctx, "c0.inspect", json!({}));
    for (name, bytes) in [
        ("../a.png", STANDARD.encode(png())),
        ("a.png", "%%%".into()),
        ("bad.png", STANDARD.encode(b"broken")),
        ("x.heic", STANDARD.encode(b"\0\0\0\x18ftypheic\0\0\0\0heicmif1")),
        ("", STANDARD.encode(png())),
    ] {
        assert_eq!(request(&mut app, &ctx, "c0.openFile", json!({"name":name,"bytes":bytes}))["ok"], false);
        assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({})), before);
    }
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({"template":"demo"}))["ok"], false);
    assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({})), before);
    app.ui.reference = Some(1);
    app.ui.dragging_photos = Some(vec![1]);
    app.ui.filmstrip = false;
    app.session.media.preview_capacity = 3;
    app.c0.chrome.community_links = false;
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({"template":"blank"}))["ok"], true);
    assert!(app.session.catalog.is_empty());
    assert!(app.ui.reference.is_none() && app.ui.dragging_photos.is_none());
    assert_eq!(app.session.media.preview_capacity, 3);
    assert!(app.session.segmenter.background);
    assert!(!app.ui.filmstrip && !app.c0.chrome.community_links);
    assert_eq!(open(&mut app, &ctx)["ok"], true, "file hooks survive a new document");
}

#[test]
fn settings_report_each_applied_unknown_and_invalid_key() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let reply = request(
        &mut app,
        &ctx,
        "c0.set",
        json!({"panels":{"top_bar":false,"left_panel":false,"filmstrip":"no","nonsense":false},"themeTokens":{"accent":"#123456","canvas":"invalid","missing":"#000000"}}),
    );
    let r = &reply["result"];
    for (kind, keys) in [
        ("applied", vec!["panels.top_bar", "panels.left_panel", "themeTokens.accent"]),
        ("unknown", vec!["panels.nonsense", "themeTokens.missing"]),
        ("invalid", vec!["panels.filmstrip", "themeTokens.canvas"]),
    ] {
        for key in keys {
            assert!(r[kind].as_array().unwrap().contains(&json!(key)), "{reply}");
        }
    }
    assert!(!app.c0.chrome.top_bar);
    assert!(!app.c0.chrome.title_bar);
    assert!(!app.ui.left_panel);
    request(&mut app, &ctx, "c0.set", json!({"panels":{"top_bar":true}}));
    assert!(app.c0.chrome.top_bar && app.c0.chrome.title_bar);
    for params in [json!({"panels":false}), json!({"themeTokens":[]})] {
        let reply = request(&mut app, &ctx, "c0.set", params);
        assert!(!reply["result"]["invalid"].as_array().unwrap().is_empty());
    }
}

#[test]
fn every_colour_token_is_overridable_across_theme_changes_and_isolated() {
    let ctx = egui::Context::default();
    let mut app = make_app();
    let source = include_str!("../src/theme.rs");
    let fields: Vec<_> = source.lines().filter_map(|l| l.trim().strip_prefix("pub ").and_then(|s| s.strip_suffix(": Color32,"))).collect();
    assert_eq!(fields.len(), 29);
    let tokens: serde_json::Map<_, _> = fields.iter().map(|f| ((*f).into(), json!("#12345678"))).collect();
    let reply = request(&mut app, &ctx, "c0.set", json!({"themeTokens":tokens}));
    assert_eq!(reply["result"]["applied"].as_array().unwrap().len(), 29);
    for kind in ThemeKind::ALL {
        theme::apply_kind(&ctx, kind);
        assert_eq!(Tokens::get(&ctx).accent, egui::Color32::from_rgba_unmultiplied(0x12, 0x34, 0x56, 0x78));
        assert_eq!(ctx.global_style().visuals.panel_fill, Tokens::get(&ctx).chrome);
    }
    let other = egui::Context::default();
    theme::apply_kind(&other, ThemeKind::Paper);
    assert_ne!(Tokens::get(&other).accent, Tokens::get(&ctx).accent);
    request(&mut app, &ctx, "c0.set", json!({"themeTokens":{"accent":null}}));
    assert_eq!(Tokens::get(&ctx).accent, Tokens::for_kind(ThemeKind::Paper).accent);
}

#[test]
fn community_policy_hides_menus_icon_about_promotions_and_contributor_links() {
    let mut app = make_app();
    app.c0.chrome.community_links = false;
    for (id, _, url) in lightcraft_ui_egui::links::LINKS {
        assert!(!lightcraft_ui_egui::menus::menu_entries(&app).iter().any(|e| e.id == *id));
        assert!(!lightcraft_ui_egui::menus::ui_enabled(&app, id));
        assert!(lightcraft_ui_egui::links::open(&mut app, url).is_err());
    }
    let mut h = Headless::new(app, [1200.0, 800.0], 1.0);
    for _ in 0..4 {
        h.step();
    }
    assert!(!h.app.widgets.iter().any(|(id, _)| id == "icon:discord"));
    h.app.run("app.about", json!({})).unwrap();
    for _ in 0..4 {
        h.step();
    }
    let text = h.painted_text().join("\n");
    assert!(text.contains("LightCraft"));
    assert!(text.contains("MIT OR Apache-2.0"));
    assert!(!text.contains("Join the ArtCraft"));
    assert!(!text.contains("ArtCraft —"));
    assert!(!h.app.widgets.iter().any(|(id, _)| id == "button:aboutDiscord"));
    h.app.ui.dialog = Some(lightcraft_ui_egui::state::Dialog::SamModel { then: None, error: None });
    for _ in 0..4 {
        h.step();
    }
    assert!(!h.app.widgets.iter().any(|(id, _)| id == "link:samHelp"));
    assert!(h.painted_text().join(" ").contains("Read the SAM License"));
}

#[test]
fn hidden_chrome_and_start_screen_are_not_painted() {
    let mut app = make_app();
    app.c0.chrome = Chrome::for_page(true);
    app.c0.chrome.tool_strip = false;
    app.c0.chrome.bottom_bar = false;
    let mut h = Headless::new(app, [1000.0, 700.0], 1.0);
    for _ in 0..4 {
        h.step();
    }
    assert!(!h.app.widgets.iter().any(|(id, _)| matches!(id.as_str(), "icon:sidebar" | "icon:discord" | "button:emptyImportPhotos" | "icon:edit")));
    assert!(!h.painted_text().join(" ").contains("Your library is empty"));
    assert_eq!(h.request("c0.cancelFileDialog", json!({}), Duration::from_secs(2))["result"]["cancelled"], false);
}

#[test]
fn save_failures_refuse_open_and_new_without_losing_the_old_library() {
    use lightcraft_catalog::{MemStore, Store};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[derive(Clone, Default)]
    struct Unplugged {
        files: MemStore,
        fail: Arc<AtomicBool>,
    }
    impl Unplugged {
        fn check(&self) -> std::io::Result<()> {
            if self.fail.load(Ordering::SeqCst) { Err(std::io::Error::other("unplugged")) } else { Ok(()) }
        }
    }
    impl Store for Unplugged {
        fn read(&mut self, n: &str) -> std::io::Result<Option<Vec<u8>>> {
            self.files.read(n)
        }
        fn write_atomic(&mut self, n: &str, b: &[u8]) -> std::io::Result<()> {
            self.check()?;
            self.files.write_atomic(n, b)
        }
        fn append(&mut self, n: &str, b: &[u8]) -> std::io::Result<()> {
            self.check()?;
            self.files.append(n, b)
        }
        fn truncate(&mut self, n: &str, l: u64) -> std::io::Result<()> {
            self.check()?;
            self.files.truncate(n, l)
        }
        fn describe(&self) -> String {
            "unplugged".into()
        }
    }
    let storage = Unplugged::default();
    let mut session = Session::new();
    session
        .open_library_in(
            lightcraft_engine::library::LibraryStores {
                dir: "c0-unsaved".into(),
                catalog: Box::new(storage.clone()),
                files: Box::new(MemStore::new()),
                on_disk: false,
            },
            true,
        )
        .unwrap();
    let mut app = LightcraftApp::new(session, Services::default());
    let ctx = egui::Context::default();
    let id = app.session.catalog.photos().next().unwrap().id.0;
    request(&mut app, &ctx, "c0.goTo", json!({"photo":id}));
    storage.fail.store(true, Ordering::SeqCst);
    let _ = app.run("photo.rate", json!({"rating":4}));
    assert!(app.session.unsaved().is_some());
    let before = app.session.catalog.len();
    assert_eq!(open(&mut app, &ctx)["ok"], false);
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({}))["ok"], false);
    assert_eq!(app.session.catalog.len(), before);
    assert_eq!(app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).unwrap().rating, 4);
    assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({}))["result"]["dirty"], true);
    storage.fail.store(false, Ordering::SeqCst);
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({}))["ok"], true);
}

#[test]
fn cancelling_a_native_picker_abandons_its_late_reply() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut rx = Some(rx);
    let mut app = make_app();
    let ctx = egui::Context::default();
    app.services.picker = Some(Box::new(move |_| rx.take().ok_or("picker already opened".into())));
    app.run("file.addPhotos", json!({})).unwrap();
    assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({}))["result"]["fileDialog"], true);
    assert_eq!(request(&mut app, &ctx, "c0.cancelFileDialog", json!({}))["result"]["cancelled"], true);
    assert!(tx.send(vec!["late.png".into()]).is_err());
    assert_eq!(request(&mut app, &ctx, "c0.inspect", json!({}))["result"]["fileDialog"], false);
    assert!(app.session.catalog.is_empty());
}

#[test]
fn failed_browser_flush_is_visible_and_blocks_library_transitions() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    assert_eq!(open(&mut app, &ctx)["ok"], true);
    let failed = std::rc::Rc::new(std::cell::Cell::new(true));
    let status = failed.clone();
    app.c0.storage_error = Some(Box::new(move || status.get().then(|| "browser quota exceeded".into())));
    let state = request(&mut app, &ctx, "c0.inspect", json!({}));
    assert_eq!(state["result"]["dirty"], true);
    assert_eq!(state["result"]["unsaved"]["storage"], true);
    assert_eq!(open(&mut app, &ctx)["ok"], false);
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({}))["ok"], false);
    assert_eq!(app.session.catalog.len(), 1);
    failed.set(false);
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({}))["ok"], true);
}

#[test]
fn byte_import_cannot_leave_an_ephemeral_original_in_a_disk_library() {
    let dir = std::env::temp_dir().join(format!("lightcraft-c0-disk-bytes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::new().with_fs();
    session.open_library(&dir, false).unwrap();
    let mut app = LightcraftApp::new(session, Services::default());
    let ctx = egui::Context::default();
    let reply = open(&mut app, &ctx);
    assert_eq!(reply["ok"], false, "{reply}");
    assert!(app.session.catalog.is_empty());
    assert_eq!(request(&mut app, &ctx, "c0.newDocument", json!({}))["ok"], true);
    assert_eq!(open(&mut app, &ctx)["ok"], true);
    std::fs::remove_dir_all(dir).unwrap();
}

// ---------- c0.selection and c0.place (the AI actions) ----------

fn solid_png(w: usize, h: usize) -> Vec<u8> {
    let image = lightcraft_raster::Rgba8::from_bytes(w, h, &[200, 120, 40, 255].repeat(w * h)).unwrap();
    lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(&image), &Default::default()).unwrap()
}

fn open_in_develop(app: &mut LightcraftApp, ctx: &egui::Context) -> u64 {
    assert_eq!(open(app, ctx)["ok"], true);
    let id = app.session.catalog.photos().next().unwrap().id.0;
    assert_eq!(request(app, ctx, "c0.goTo", json!({"photo":id,"view":"detail"}))["ok"], true);
    id
}

fn decode_png(text: &str) -> (u32, u32, Vec<u8>) {
    let bytes = STANDARD.decode(text).unwrap();
    assert_eq!(&bytes[1..4], b"PNG");
    let img = lightcraft_codecs::decode(&bytes, Default::default()).unwrap();
    (img.image.width as u32, img.image.height as u32, bytes)
}

#[test]
fn selection_needs_a_photo_open_in_develop() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    assert_eq!(request(&mut app, &ctx, "c0.selection", json!({}))["ok"], false);
    assert_eq!(open(&mut app, &ctx)["ok"], true);
    let id = app.session.catalog.photos().next().unwrap().id.0;
    request(&mut app, &ctx, "c0.goTo", json!({"photo":id,"view":"grid"}));
    let reply = request(&mut app, &ctx, "c0.selection", json!({}));
    assert_eq!(reply["ok"], false, "{reply}");
    assert!(reply["error"].as_str().unwrap().contains("Develop"));
    request(&mut app, &ctx, "c0.goTo", json!({"photo":id,"view":"detail"}));
    assert_eq!(request(&mut app, &ctx, "c0.selection", json!({"photo":id + 99}))["ok"], false);
    assert_eq!(request(&mut app, &ctx, "c0.selection", json!({"photo":"x"}))["ok"], false);
}

#[test]
fn selection_reads_the_developed_photo_and_the_active_mask() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let id = open_in_develop(&mut app, &ctx);
    let before = request(&mut app, &ctx, "c0.inspect", json!({}));
    let reply = request(&mut app, &ctx, "c0.selection", json!({}));
    assert_eq!(reply["ok"], true, "{reply}");
    let r = &reply["result"];
    assert_eq!((r["kind"].as_str(), r["photoId"].as_u64(), r["name"].as_str()), (Some("photo"), Some(id), Some("original.png")));
    let (w, h, _) = decode_png(r["png"].as_str().unwrap());
    assert_eq!((r["width"].as_u64(), r["height"].as_u64()), (Some(w as u64), Some(h as u64)));
    assert!(r.get("mask").is_none(), "no mask is selected");
    // the picture is the photo as developed: an exposure change shows in it
    app.run("develop.set", json!({"control":"light.exposure","value":1.5})).unwrap();
    let edited = request(&mut app, &ctx, "c0.selection", json!({}));
    assert_ne!(edited["result"]["png"], r["png"]);
    // an active mask comes along as a PNG of the same size
    app.run("mask.add", json!({"kind":"radial"})).unwrap();
    let masked = request(&mut app, &ctx, "c0.selection", json!({}));
    let mask = masked["result"]["mask"].as_str().expect("mask png");
    let (mw, mh, _) = decode_png(mask);
    assert_eq!((mw, mh), (w, h));
    let state = request(&mut app, &ctx, "c0.inspect", json!({}));
    assert_eq!(state["result"]["photoCount"], before["result"]["photoCount"], "reading changes nothing");
}

#[test]
fn selection_picture_is_at_most_2048_on_the_long_side() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let name = "wide.png";
    let reply = request(&mut app, &ctx, "c0.openFile", json!({"name":name,"bytes":STANDARD.encode(solid_png(3000, 40))}));
    assert_eq!(reply["ok"], true, "{reply}");
    let id = app.session.catalog.photos().next().unwrap().id.0;
    request(&mut app, &ctx, "c0.goTo", json!({"photo":id,"view":"detail"}));
    let r = request(&mut app, &ctx, "c0.selection", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    let (w, h, _) = decode_png(r["result"]["png"].as_str().unwrap());
    assert!(w.max(h) <= 2048 && w > h, "{w}x{h}");
    assert_eq!(r["result"]["sourceWidth"], 3000);
}

#[test]
fn place_adds_a_new_version_stacked_next_to_the_source_as_one_undo_step() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let id = open_in_develop(&mut app, &ctx);
    let original = app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).unwrap().source.clone();
    let steps = app.session.undo.len();
    for (key, name, data) in [("png", "ai.png", solid_png(4, 3))] {
        let reply = request(&mut app, &ctx, "c0.place", json!({key: STANDARD.encode(data), "name": name, "mode": "new-version"}));
        assert_eq!(reply["ok"], true, "{reply}");
        let r = &reply["result"];
        let new = r["photoId"].as_u64().unwrap();
        assert_ne!(new, id);
        assert_eq!((r["placed"].as_str(), r["stackedWith"].as_u64()), (Some("photo"), Some(id)));
        let catalog = &app.session.catalog;
        assert_eq!(catalog.photos().filter(|p| p.in_library()).count(), 2);
        let stack = catalog.stack_of(lightcraft_catalog::PhotoId(id)).expect("stacked with the source");
        assert!(stack.photos.iter().any(|p| p.0 == new));
        assert_eq!(catalog.photo(lightcraft_catalog::PhotoId(id)).unwrap().source, original, "the original file is untouched");
        assert_eq!(app.session.undo.len(), steps + 1, "one undo step");
        // one Ctrl+Z removes the picture and the stack
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(app.session.catalog.photos().filter(|p| p.in_library()).count(), 1);
        assert!(app.session.catalog.stack_of(lightcraft_catalog::PhotoId(id)).is_none());
        assert_eq!(app.session.undo.len(), steps);
    }
}

#[test]
fn place_takes_a_jpeg_and_marks_the_document_changed() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let id = open_in_develop(&mut app, &ctx);
    let image = lightcraft_raster::Rgba8::from_bytes(8, 8, &[10, 200, 90, 255].repeat(64)).unwrap();
    let jpeg = lightcraft_codecs::encode_jpeg(&lightcraft_codecs::EncodeImage::rgba8(&image), 90, Default::default(), &Default::default()).unwrap();
    let reply = request(&mut app, &ctx, "c0.place", json!({"jpeg": STANDARD.encode(jpeg), "mode": "new-version", "source": id}));
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(reply["result"]["name"], "ai-result.jpg");
    let state = request(&mut app, &ctx, "c0.inspect", json!({}));
    assert_eq!(state["result"]["photoCount"], 2);
    assert_eq!(state["result"]["canUndo"], true);
}

#[test]
fn place_with_bad_params_answers_a_plain_error_and_changes_nothing() {
    let mut app = make_app();
    let ctx = egui::Context::default();
    let id = open_in_develop(&mut app, &ctx);
    let good = STANDARD.encode(solid_png(4, 3));
    let steps = app.session.undo.len();
    for params in [
        json!({}),
        json!({"png": good, "mode": "replace"}),
        json!({"png": good}),
        json!({"mode": "new-version"}),
        json!({"png": good, "jpeg": good, "mode": "new-version"}),
        json!({"png": 5, "mode": "new-version"}),
        json!({"png": "!!!not base64", "mode": "new-version"}),
        json!({"png": STANDARD.encode(b"not a picture"), "mode": "new-version"}),
        json!({"png": good, "mode": "new-version", "name": "../x.png"}),
        json!({"png": good, "mode": "new-version", "name": 7}),
        json!({"png": good, "mode": "new-version", "source": 424242}),
        json!({"png": "A".repeat(60 * 1024 * 1024), "mode": "new-version"}),
    ] {
        let reply = request(&mut app, &ctx, "c0.place", params);
        assert_eq!(reply["ok"], false, "{reply}");
        assert!(reply["error"].as_str().is_some_and(|e| !e.is_empty()));
    }
    assert_eq!(app.session.catalog.photos().filter(|p| p.in_library()).count(), 1);
    assert_eq!(app.session.undo.len(), steps);
    assert!(app.session.catalog.stack_of(lightcraft_catalog::PhotoId(id)).is_none());
    // the same picture twice: the duplicate is refused and the first placement stays
    let first = request(&mut app, &ctx, "c0.place", json!({"png": good, "mode": "new-version"}));
    assert_eq!(first["ok"], true, "{first}");
    let again = request(&mut app, &ctx, "c0.place", json!({"png": good, "mode": "new-version"}));
    assert_eq!(again["ok"], false, "{again}");
    assert_eq!(app.session.catalog.photos().filter(|p| p.in_library()).count(), 2);
}
