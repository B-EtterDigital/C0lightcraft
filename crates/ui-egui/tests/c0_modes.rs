//! Tool-strip modes (`toolbar`, `toolbar_double`, `toolbar_floating`) and the folded right dock.

use lightcraft_engine::Session;
use lightcraft_ui_egui::{
    LightcraftApp, Services,
    control::{self, ControlRequest, Outcome},
    headless::Headless,
    state::RightPanel,
};
use serde_json::{Value, json};

const SIZE: [f32; 2] = [1000.0, 700.0];
const ICONS: [&str; 10] = ["presets", "edit", "crop", "remove", "masking", "redeye", "versions", "activity", "keywords", "info"];

fn make_app() -> LightcraftApp {
    LightcraftApp::new(Session::new(), Services::default())
}

fn request(app: &mut LightcraftApp, ctx: &egui::Context, method: &str, params: Value) -> Value {
    let (req, _) = ControlRequest::new(method, params);
    match control::handle(app, ctx, &req) {
        Outcome::Done(value) => value,
        Outcome::Screenshot { .. } => unreachable!("no screenshot requested"),
        Outcome::Gesture { .. } => unreachable!("no gesture requested"),
    }
}

fn set(app: &mut LightcraftApp, panels: Value) -> Value {
    let ctx = egui::Context::default();
    request(app, &ctx, "c0.set", json!({"panels": panels}))["result"].clone()
}

fn panes(app: &mut LightcraftApp) -> Value {
    let ctx = egui::Context::default();
    request(app, &ctx, "c0.inspect", json!({}))["result"]["panes"].clone()
}

fn headless(app: LightcraftApp) -> Headless {
    let mut h = Headless::new(app, SIZE, 1.0);
    frames(&mut h);
    h
}

fn frames(h: &mut Headless) {
    for _ in 0..5 {
        h.step();
    }
}

fn rect(h: &Headless, id: &str) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == id).map(|(_, r)| *r)
}

fn icon(h: &Headless, name: &str) -> egui::Rect {
    rect(h, &format!("icon:{name}")).unwrap_or_else(|| panic!("icon:{name} not drawn"))
}

fn all_icons(h: &Headless) -> Vec<egui::Rect> {
    ICONS.iter().map(|n| icon(h, n)).collect()
}

fn assert_no_overlap(rects: &[egui::Rect]) {
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            assert!(a.intersect(*b).area() < 1.0, "{a:?} overlaps {b:?}");
        }
    }
}

fn columns(rects: &[egui::Rect]) -> usize {
    let mut xs: Vec<f32> = rects.iter().map(|r| r.center().x).collect();
    xs.sort_by(f32::total_cmp);
    // (the bottom pair sits a pixel off the top group's rows)
    xs.windows(2).filter(|w| w[1] - w[0] > 4.0).count() + 1
}

#[test]
fn defaults_equal_upstream() {
    let mut app = make_app();
    let p = panes(&mut app);
    assert_eq!(p["toolbar"], true);
    assert_eq!(p["toolbar_double"], false);
    assert_eq!(p["toolbar_floating"], false);
    assert_eq!(p["right_dock_collapsed"], false);
    assert_eq!(p["tool_strip"], true);
    let h = headless(app);
    let icons = all_icons(&h);
    assert_eq!(columns(&icons), 1, "one docked column");
    assert!(icons.iter().all(|r| r.right() <= SIZE[0] + 1.0));
    assert!(icon(&h, "edit").right() >= SIZE[0] - 1.0, "docked at the right edge");
    assert!(rect(&h, "panel:right_panel").is_some(), "the right dock is open");
}

#[test]
fn toolbar_aliases_the_tool_strip_switch() {
    let mut app = make_app();
    let r = set(&mut app, json!({"toolbar": false}));
    assert_eq!(r["applied"], json!(["panels.toolbar"]));
    let p = panes(&mut app);
    assert_eq!((p["toolbar"].clone(), p["tool_strip"].clone()), (json!(false), json!(false)));
    let h = headless(app);
    assert!(rect(&h, "icon:edit").is_none());
}

#[test]
fn double_docks_the_strip_in_two_columns() {
    let mut app = make_app();
    let r = set(&mut app, json!({"toolbar_double": true}));
    assert_eq!((r["applied"].clone(), r["unknown"].clone(), r["invalid"].clone()), (json!(["panels.toolbar_double"]), json!([]), json!([])));
    assert_eq!(panes(&mut app)["toolbar_double"], true);
    let h = headless(app);
    let icons = all_icons(&h);
    assert_eq!(columns(&icons), 2);
    assert_no_overlap(&icons);
    assert_eq!(icon(&h, "presets").center().y, icon(&h, "edit").center().y, "two to a row");
    assert!(icon(&h, "presets").center().x < icon(&h, "edit").center().x);
    assert!(icon(&h, "edit").right() >= SIZE[0] - 1.0, "still docked at the right edge");
    assert!(icon(&h, "info").bottom() > SIZE[1] - 60.0, "Keywords and Info stay at the bottom");
    assert!(icons.iter().all(|r| r.left() >= 0.0 && r.bottom() <= SIZE[1]), "{icons:?}");
}

#[test]
fn floating_frees_the_right_edge_and_stays_on_screen() {
    let mut app = make_app();
    let r = set(&mut app, json!({"toolbar_floating": true}));
    assert_eq!(r["applied"], json!(["panels.toolbar_floating"]));
    assert_eq!(panes(&mut app)["toolbar_floating"], true);
    let h = headless(app);
    let icons = all_icons(&h);
    assert_eq!(columns(&icons), 1, "floating, one column");
    assert_no_overlap(&icons);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE[0], SIZE[1]));
    assert!(icons.iter().all(|r| screen.contains_rect(*r)));
    // the dock keeps its width, so the palette sits beside it instead of at the window edge
    let dock = rect(&h, "panel:right_panel").expect("dock");
    assert!(icon(&h, "edit").right() <= dock.left(), "palette left of the open dock");
    assert!(icon(&h, "edit").right() < SIZE[0] - 1.0);
}

#[test]
fn floating_double_is_a_two_column_palette() {
    let mut app = make_app();
    set(&mut app, json!({"toolbar_floating": true, "toolbar_double": true}));
    let h = headless(app);
    let icons = all_icons(&h);
    assert_eq!(columns(&icons), 2);
    assert_no_overlap(&icons);
    assert_eq!(icon(&h, "presets").center().y, icon(&h, "edit").center().y);
    assert!(icon(&h, "edit").right() < SIZE[0] - 1.0);
}

#[test]
fn modes_switch_back_to_the_docked_column() {
    let mut app = make_app();
    set(&mut app, json!({"toolbar_floating": true, "toolbar_double": true}));
    let mut h = headless(app);
    set(&mut h.app, json!({"toolbar_floating": false, "toolbar_double": false}));
    frames(&mut h);
    assert_eq!(columns(&all_icons(&h)), 1);
    assert!(icon(&h, "edit").right() >= SIZE[0] - 1.0);
}

#[test]
fn unknown_and_invalid_keys_are_reported() {
    let mut app = make_app();
    let r =
        set(&mut app, json!({"toolbar_triple": true, "toolbar_double": "yes", "toolbar_floating": 1, "right_dock_collapsed": null, "toolbar": true}));
    assert_eq!(r["unknown"], json!(["panels.toolbar_triple"]));
    let invalid = r["invalid"].as_array().cloned().unwrap_or_default();
    for key in ["panels.toolbar_double", "panels.toolbar_floating", "panels.right_dock_collapsed"] {
        assert!(invalid.contains(&json!(key)), "{key} in {invalid:?}");
    }
    assert_eq!(r["applied"], json!(["panels.toolbar"]));
    let p = panes(&mut app);
    assert_eq!(
        (p["toolbar_double"].clone(), p["toolbar_floating"].clone(), p["right_dock_collapsed"].clone()),
        (json!(false), json!(false), json!(false))
    );
}

#[test]
fn modes_are_not_part_of_the_saved_ui_state() {
    let mut app = make_app();
    set(&mut app, json!({"toolbar_double": true, "toolbar_floating": true, "right_dock_collapsed": true}));
    let saved = serde_json::to_string(&app.ui).unwrap_or_default().to_lowercase();
    assert!(!saved.is_empty());
    for needle in ["toolbar", "rightdockcollapsed", "right_dock_collapsed", "floating"] {
        assert!(!saved.contains(needle), "{needle} leaked into prefs");
    }
}

#[test]
fn the_right_dock_folds_to_the_icon_strip_and_unfolds() {
    let mut app = make_app();
    app.ui.right = RightPanel::Crop;
    let mut h = headless(app);
    assert!(rect(&h, "panel:right_panel").is_some());
    let r = set(&mut h.app, json!({"right_dock_collapsed": true}));
    assert_eq!(r["applied"], json!(["panels.right_dock_collapsed"]));
    assert_eq!(panes(&mut h.app)["right_dock_collapsed"], true);
    frames(&mut h);
    assert!(rect(&h, "panel:right_panel").is_none(), "no dock while folded");
    assert_eq!(columns(&all_icons(&h)), 1, "the icon strip stays");
    // folding twice changes nothing
    set(&mut h.app, json!({"right_dock_collapsed": true}));
    set(&mut h.app, json!({"right_dock_collapsed": false}));
    assert_eq!(h.app.ui.right, RightPanel::Crop, "unfold brings back the panel that was open");
    frames(&mut h);
    assert!(rect(&h, "panel:right_panel").is_some());
    assert_eq!(panes(&mut h.app)["right_dock_collapsed"], false);
}

#[test]
fn a_strip_icon_opens_a_folded_dock() {
    let mut app = make_app();
    let mut h = {
        app.ui.right = RightPanel::Edit;
        headless(app)
    };
    set(&mut h.app, json!({"right_dock_collapsed": true}));
    frames(&mut h);
    assert!(h.app.run("panel.info", json!({})).is_ok());
    assert_eq!(h.app.ui.right, RightPanel::Info);
    assert_eq!(panes(&mut h.app)["right_dock_collapsed"], false);
    frames(&mut h);
    assert!(rect(&h, "panel:right_panel").is_some());
    // a panel opened then closed again by hand is a closed dock, not a stale fold
    assert!(h.app.run("panel.info", json!({})).is_ok());
    assert_eq!(h.app.ui.right, RightPanel::None);
    assert_eq!(panes(&mut h.app)["right_dock_collapsed"], false);
}

#[test]
fn folding_an_already_closed_dock_unfolds_to_edit() {
    let mut app = make_app();
    app.ui.right = RightPanel::None;
    set(&mut app, json!({"right_dock_collapsed": true}));
    assert_eq!(panes(&mut app)["right_dock_collapsed"], true);
    set(&mut app, json!({"right_dock_collapsed": false}));
    assert_eq!(app.ui.right, RightPanel::Edit);
}

#[test]
fn fold_keeps_modes_independent() {
    let mut app = make_app();
    set(&mut app, json!({"toolbar_double": true, "right_dock_collapsed": true}));
    let p = panes(&mut app);
    assert_eq!(
        (p["toolbar_double"].clone(), p["right_dock_collapsed"].clone(), p["toolbar_floating"].clone()),
        (json!(true), json!(true), json!(false))
    );
    let h = headless(app);
    assert_eq!(columns(&all_icons(&h)), 2);
}
