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
    assert_eq!(fields.len(), 28);
    let tokens: serde_json::Map<_, _> = fields.iter().map(|f| ((*f).into(), json!("#12345678"))).collect();
    let reply = request(&mut app, &ctx, "c0.set", json!({"themeTokens":tokens}));
    assert_eq!(reply["result"]["applied"].as_array().unwrap().len(), 28);
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
