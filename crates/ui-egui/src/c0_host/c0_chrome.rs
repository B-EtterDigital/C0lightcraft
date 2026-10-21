//! Unsaved host policy; every default preserves upstream behaviour.

use crate::LightcraftApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub title_bar: bool,
    pub top_bar: bool,
    pub start_screen: bool,
    pub community_links: bool,
    pub experimental_notice: bool,
    pub tool_strip: bool,
    pub bottom_bar: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            title_bar: true,
            top_bar: true,
            start_screen: true,
            community_links: true,
            experimental_notice: true,
            tool_strip: true,
            bottom_bar: true,
        }
    }
}

impl Chrome {
    pub fn for_page(hosted: bool) -> Self {
        if hosted {
            Self { title_bar: false, top_bar: false, start_screen: false, community_links: false, experimental_notice: false, ..Self::default() }
        } else {
            Self::default()
        }
    }
}

pub fn community_url(url: &str) -> bool {
    let host = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).and_then(|s| s.split('/').next()).unwrap_or("");
    matches!(host, "github.com" | "getartcraft.com" | "www.getartcraft.com" | "discord.gg" | "discord.com")
}

pub fn contributor_link(app: &LightcraftApp, ui: &mut egui::Ui, name: &str) -> egui::Response {
    if app.c0.chrome.community_links { ui.link(name) } else { ui.label(name) }
}

pub fn external_link(app: &LightcraftApp, ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, url: &str) -> egui::Response {
    if !app.c0.chrome.community_links && community_url(url) { ui.label(label) } else { ui.link(label).on_hover_text(url) }
}

/// Upstream's original About links, isolated so the policy costs one upstream call site.
pub fn about_links(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    if !app.c0.chrome.community_links {
        return;
    }
    ui.add_space(10.0);
    let r = crate::button::Button::new("Join the ArtCraft Discord")
        .primary()
        .large()
        .min_width(260.0)
        .widget("button:aboutDiscord")
        .tooltip(crate::links::DISCORD)
        .show(ui);
    if r.clicked() {
        let _ = crate::links::open(app, crate::links::DISCORD);
    }
    ui.add_space(6.0);
    for (label, url) in [
        ("LightCraft website", crate::links::APP_PAGE),
        ("Source code on GitHub", crate::links::GITHUB),
        ("ArtCraft — more creative apps", crate::links::WEBSITE),
    ] {
        if ui.link(crate::i18n::tr(label)).on_hover_text(url).clicked() {
            let _ = crate::links::open(app, url);
        }
    }
}
