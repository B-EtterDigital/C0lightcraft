//! Tool-strip layouts for the guest page (docked or floating, one or two columns) and the folded
//! right dock. Nothing here is saved: a fresh app starts with upstream's single docked column.

use egui::{Id, Order, Sense, UiBuilder, pos2, vec2};

use crate::LightcraftApp;
use crate::state::RightPanel;
use crate::theme::Tokens;

/// Height of one tool button and the gap between them (as in `panels::strip`).
const BUTTON_H: f32 = 40.0;
/// The palette's drag handle.
const GRIP_H: f32 = 12.0;
/// Palette heights that hold every tool button plus the bottom pair, per column count.
const HEIGHT_ONE: f32 = 540.0;
const HEIGHT_TWO: f32 = 340.0;
const MARGIN: f32 = 8.0;

#[derive(Default)]
pub struct Modes {
    pub double: bool,
    pub floating: bool,
    drawing_palette: bool,
    /// The panel to bring back when the right dock unfolds; `Some` while folded.
    folded: Option<RightPanel>,
}

impl Modes {
    pub fn cols(&self) -> f32 {
        if self.double { 2.0 } else { 1.0 }
    }
}

/// A folded dock is a closed dock: a panel opened since (icon, shortcut) ends the fold.
pub(crate) fn reconcile(app: &mut LightcraftApp) {
    if app.ui.right != RightPanel::None {
        app.c0.modes.folded = None;
    }
}

pub(crate) fn collapsed(app: &LightcraftApp) -> bool {
    app.c0.modes.folded.is_some() && app.ui.right == RightPanel::None
}

/// Fold the right dock to the tool strip, or bring the folded panel back.
pub(crate) fn set_collapsed(app: &mut LightcraftApp, on: bool) {
    reconcile(app);
    if on && !collapsed(app) {
        app.c0.modes.folded = Some(if app.ui.right == RightPanel::None { RightPanel::Edit } else { app.ui.right });
        // (what the dock's own close does to a running tool)
        let _ = app.session.end_interaction();
        if app.ui.tool != "wbPicker" {
            app.ui.tool.clear();
        }
        app.ui.right = RightPanel::None;
    } else if !on && let Some(panel) = app.c0.modes.folded.take() {
        app.ui.right = panel;
    }
}

/// The strip's buttons in one centred column, or wrapped two to a row.
pub(crate) fn flow(ui: &mut egui::Ui, cols: f32, bottom: bool, add: impl FnOnce(&mut egui::Ui)) {
    if cols < 2.0 {
        ui.vertical_centered(add);
        return;
    }
    if bottom {
        let spare = ui.available_height() - BUTTON_H;
        if spare > 0.0 {
            ui.add_space(spare);
        }
    }
    // (the panel's 1 px outline eats into its width: widen the row so two buttons fit)
    let row = ui.available_rect_before_wrap().expand2(vec2(2.0, 0.0));
    let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
    ui.scope_builder(UiBuilder::new().max_rect(row).layout(layout), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        add(ui);
    });
}

/// The rule between tool groups; when the buttons wrap it is too wide to share a row.
pub(crate) fn separator(ui: &mut egui::Ui, strip_w: f32, color: egui::Color32) {
    let wrapped = ui.layout().main_wrap;
    let width = if wrapped { ui.max_rect().width() - 16.0 } else { strip_w - 16.0 };
    let (r, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, color.gamma_multiply(0.7));
}

/// Draw the strip as a movable palette over the canvas. Returns whether it did, so the docked
/// panel is skipped and the canvas keeps the full width.
pub(crate) fn float(app: &mut LightcraftApp, ui: &mut egui::Ui) -> bool {
    reconcile(app);
    if !app.c0.modes.floating || app.c0.modes.drawing_palette {
        return false;
    }
    let ctx = ui.ctx().clone();
    let t = Tokens::get(&ctx);
    let area = ui.available_rect_before_wrap();
    let width = t.strip_w * app.c0.modes.cols();
    let tools_h = if app.c0.modes.double { HEIGHT_TWO } else { HEIGHT_ONE };
    let height = (tools_h + GRIP_H).min((area.height() - 2.0 * MARGIN).max(GRIP_H));
    // beside the open right dock, not over it
    let dock = if app.ui.right == RightPanel::None { 0.0 } else { app.ui.right_width };
    let start = pos2((area.right() - width - dock - MARGIN).max(area.left()), area.top() + MARGIN);
    app.c0.modes.drawing_palette = true;
    egui::Area::new(Id::new("c0-tool-palette")).order(Order::Middle).default_pos(start).constrain_to(area).movable(true).show(&ctx, |ui| {
        let (grip, _) = ui.allocate_exact_size(vec2(width, GRIP_H), Sense::hover());
        ui.painter().rect_filled(grip, 0.0, t.chrome);
        ui.painter().line_segment([grip.center() - vec2(10.0, 0.0), grip.center() + vec2(10.0, 0.0)], egui::Stroke::new(2.0, t.text_dim));
        let (rect, _) = ui.allocate_exact_size(vec2(width, height - GRIP_H), Sense::hover());
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
        crate::panels::strip::show(app, &mut child);
    });
    app.c0.modes.drawing_palette = false;
    true
}
