//! UI after the shared design system v2.0 (`ui_design_system_v2.0.md`): light and dark mode with the same token names,
//! slate blue only for selection, interaction and progress, status colors only for states. Dark mode is neutral grey;
//! the area behind the document stays neutral so a page gets no color cast. Sizes on the 4 px grid, all controls in a
//! row 32 px high.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32, CornerRadius, Margin, Response, Shadow, Stroke, Theme, ThemePreference, Visuals};

/// Colors of the design system. The functions return the value of the currently active appearance.
pub mod tokens {
    use super::*;

    /// One appearance (light or dark); names match the semantic tokens of the design system (§2).
    pub struct Palette {
        /// Window, recessed fields.
        pub bg: Color32,
        /// Neutral area behind the document page.
        pub canvas: Color32,
        /// Bars and panels.
        pub surface: Color32,
        /// Cards, lists, dialogs, notices – always with `shadow`.
        pub surface_raised: Color32,
        /// Secondary button, chips, table header.
        pub surface_muted: Color32,
        /// Area under the pointer (neutral; blue means "on").
        pub hover: Color32,
        pub surface_pressed: Color32,
        /// Separator lines, fine borders.
        pub line: Color32,
        /// Border of checkboxes, radio buttons and input fields (≥ 3:1).
        pub line_strong: Color32,
        pub text: Color32,
        /// Secondary text and icons at rest (≥ 4.5:1 on `surface`).
        pub muted: Color32,
        /// Disabled (exempt from 4.5:1).
        pub disabled: Color32,
        /// Selection border, tab line, progress, links.
        pub accent: Color32,
        /// Active icons, check marks, focus ring, content of selected segments.
        pub accent_strong: Color32,
        /// "On", highlighted row, selected segments – on it only `text`, icons or `accent_strong`.
        pub accent_subtle: Color32,
        /// Larger highlighted area with secondary text (filing target, selected row), always with an accent border.
        pub highlight: Color32,
        /// Primary button; white on it ≥ 4.5:1.
        pub accent_fill: Color32,
        pub accent_fill_hover: Color32,
        /// Button for irreversible actions (delete).
        pub danger_fill: Color32,
        pub danger_fill_hover: Color32,
        pub ok: Color32,
        pub ok_bg: Color32,
        pub warn: Color32,
        pub warn_bg: Color32,
        pub error: Color32,
        pub error_bg: Color32,
        /// Dimming behind dialog cards.
        pub scrim: Color32,
        pub shadow: Shadow,
        /// Text selection in input fields. Not a design system token: `accent_subtle` would be invisible on a white
        /// field.
        pub selection: Color32,
    }

    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// Color with opacity `alpha` (0–255), premultiplied as egui expects.
    const fn rgba(hex: u32, alpha: u32) -> Color32 {
        let r = ((hex >> 16) & 0xFF) * alpha / 255;
        let g = ((hex >> 8) & 0xFF) * alpha / 255;
        let b = (hex & 0xFF) * alpha / 255;
        Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, alpha as u8)
    }

    /// §2.3
    pub const LIGHT: Palette = Palette {
        bg: rgb(0xF2F4F7),
        canvas: rgb(0xE3E3E3),
        surface: rgb(0xFFFFFF),
        surface_raised: rgb(0xFFFFFF),
        surface_muted: rgb(0xF8F9FB),
        hover: rgb(0xEEF2F7),
        surface_pressed: rgb(0xE3E9F0),
        line: rgb(0xD4DCE6),
        line_strong: rgb(0x7F8FA3),
        text: rgb(0x1A2330),
        muted: rgb(0x52637A),
        disabled: rgb(0x9AA5B3),
        accent: rgb(0x3B5F8A),
        accent_strong: rgb(0x2D4D73),
        accent_subtle: rgb(0xEBF0F7),
        highlight: rgb(0xEBF0F7),
        accent_fill: rgb(0x3B5F8A),
        accent_fill_hover: rgb(0x2D4D73),
        danger_fill: rgb(0xC0392B),
        danger_fill_hover: rgb(0xA93226),
        ok: rgb(0x17784F),
        ok_bg: rgb(0xE6F4EE),
        warn: rgb(0xAD5A00),
        warn_bg: rgb(0xFEF3E6),
        error: rgb(0xC0392B),
        error_bg: rgb(0xFDECEB),
        scrim: rgba(0x1A2330, 115),
        shadow: Shadow { offset: [0, 10], blur: 28, spread: 0, color: rgba(0x1A2330, 20) },
        selection: rgb(0xC6D6EA),
    };

    /// §2.2
    pub const DARK: Palette = Palette {
        bg: rgb(0x141414),
        canvas: rgb(0x141414),
        surface: rgb(0x202020),
        surface_raised: rgb(0x262626),
        surface_muted: rgb(0x2A2A2A),
        hover: rgb(0x2A2A2A),
        surface_pressed: rgb(0x333333),
        line: rgb(0x353535),
        line_strong: rgb(0x707070),
        text: rgb(0xE6E6E6),
        muted: rgb(0x9A9A9A),
        disabled: rgb(0x5A5A5A),
        accent: rgb(0x5B8EC4),
        accent_strong: rgb(0x7AACD8),
        accent_subtle: rgb(0x1E3A52),
        highlight: rgb(0x162638),
        accent_fill: rgb(0x457AAE),
        accent_fill_hover: rgb(0x3F6E9E),
        danger_fill: rgb(0xC94A3F),
        danger_fill_hover: rgb(0xB4433A),
        ok: rgb(0x2ECC8B),
        ok_bg: rgb(0x0E3326),
        warn: rgb(0xF0A330),
        warn_bg: rgb(0x3A2600),
        error: rgb(0xE66A5E),
        error_bg: rgb(0x3D1210),
        scrim: rgba(0x000000, 166),
        shadow: Shadow { offset: [0, 8], blur: 24, spread: 0, color: rgba(0x000000, 102) },
        selection: rgb(0x2B527A),
    };

    static LIGHT_ACTIVE: AtomicBool = AtomicBool::new(false);

    /// Which appearance is active; `theme::sync` sets it at the start of every frame.
    pub fn set_light(light: bool) {
        LIGHT_ACTIVE.store(light, Ordering::Relaxed);
    }

    pub fn is_light() -> bool {
        LIGHT_ACTIVE.load(Ordering::Relaxed)
    }

    pub fn palette() -> &'static Palette {
        if is_light() { &LIGHT } else { &DARK }
    }

    pub fn bg() -> Color32 {
        palette().bg
    }
    pub fn surface() -> Color32 {
        palette().surface
    }
    pub fn surface_raised() -> Color32 {
        palette().surface_raised
    }
    pub fn surface_muted() -> Color32 {
        palette().surface_muted
    }
    /// Area under the pointer.
    pub fn hover() -> Color32 {
        palette().hover
    }
    /// Primary text.
    pub fn fg() -> Color32 {
        palette().text
    }
    /// Secondary text and icons at rest (at least 4.5:1 on `surface`).
    pub fn muted() -> Color32 {
        palette().muted
    }
    pub fn disabled() -> Color32 {
        palette().disabled
    }
    pub fn line() -> Color32 {
        palette().line
    }
    pub fn line_strong() -> Color32 {
        palette().line_strong
    }
    pub fn accent() -> Color32 {
        palette().accent
    }
    pub fn accent_strong() -> Color32 {
        palette().accent_strong
    }
    pub fn accent_subtle() -> Color32 {
        palette().accent_subtle
    }
    /// Area for highlighted regions with text (filing target, selected row); always with an accent border.
    pub fn highlight() -> Color32 {
        palette().highlight
    }
    pub fn status_ok() -> Color32 {
        palette().ok
    }
    pub fn status_ok_bg() -> Color32 {
        palette().ok_bg
    }
    pub fn status_err() -> Color32 {
        palette().error
    }
    pub fn status_err_bg() -> Color32 {
        palette().error_bg
    }
    pub fn status_warn() -> Color32 {
        palette().warn
    }
    pub fn status_warn_bg() -> Color32 {
        palette().warn_bg
    }
    pub fn canvas() -> Color32 {
        palette().canvas
    }
    pub fn scrim() -> Color32 {
        palette().scrim
    }
    pub fn shadow() -> Shadow {
        palette().shadow
    }
}

/// Lucide icons (https://lucide.dev, ISC license, see `assets/lucide`) as characters of the embedded icon font. They
/// sit in the text like letters and take its color (outline, no fill).
pub mod icon {
    pub const FOLDER: char = '\u{e0dc}';
    pub const FOLDER_OPEN: char = '\u{e247}';
    pub const FOLDER_PLUS: char = '\u{e0de}';
    pub const FOLDER_SEARCH: char = '\u{e33d}';
    pub const FILE_TEXT: char = '\u{e0d0}';
    pub const FILE_PLUS: char = '\u{e0cd}';
    pub const SETTINGS: char = '\u{e158}';
    pub const SUN: char = '\u{e17c}';
    pub const MOON: char = '\u{e122}';
    pub const SUN_MOON: char = '\u{e2b2}';
    pub const X: char = '\u{e1b2}';
    pub const TRASH: char = '\u{e18e}';
    pub const REFRESH: char = '\u{e149}';
    pub const SEARCH: char = '\u{e155}';
    pub const PLUS: char = '\u{e141}';
    pub const WARN: char = '\u{e193}';
    pub const ERROR: char = '\u{e07b}';
    pub const OK: char = '\u{e226}';
    pub const UNDO: char = '\u{e2a1}';
    pub const CHEVRON_RIGHT: char = '\u{e073}';
    pub const CHEVRON_DOWN: char = '\u{e071}';
    pub const CHEVRON_LEFT: char = '\u{e072}';
    pub const CHEVRON_UP: char = '\u{e074}';
    pub const EXTERNAL: char = '\u{e0bd}';
    pub const PANEL_CLOSE: char = '\u{e21c}';
    pub const PANEL_OPEN: char = '\u{e21d}';
    pub const PAUSE: char = '\u{e132}';
    pub const PLAY: char = '\u{e140}';
    pub const ZOOM_IN: char = '\u{e1b6}';
    pub const ZOOM_OUT: char = '\u{e1b7}';
    pub const ROTATE_CCW: char = '\u{e14c}';
    pub const ROTATE_CW: char = '\u{e14d}';
    pub const SPARKLES: char = '\u{e417}';
    pub const DOWNLOAD: char = '\u{e0b6}';
    pub const ERASER: char = '\u{e28f}';
    pub const LIST_X: char = '\u{e240}';
    pub const SCAN: char = '\u{e53d}';
}

/// Label with a leading icon, e.g. for buttons.
pub fn with_icon(glyph: char, label: &str) -> String {
    format!("{glyph}  {label}")
}

/// An icon in a grid size (16 inline, 20 in bars, 24 in cards, 32 in the empty state).
pub fn icon_text(glyph: char, size: f32, color: Color32) -> egui::RichText {
    egui::RichText::new(glyph.to_string()).size(size).color(color)
}

/// Font sizes after the design system (§3).
pub mod text {
    /// Section and field labels: capitals, semibold, letter-spaced.
    pub const OVERLINE: f32 = 11.0;
    /// Second lines, abbreviations, table header.
    pub const SMALL: f32 = 12.0;
    pub const BODY: f32 = 13.0;
    pub const LARGE: f32 = 15.0;
    /// Key figures, card titles, dialog titles.
    pub const VALUE: f32 = 17.0;
    pub const TITLE: f32 = 22.0;
    /// Monospace: paths, raw data, lists.
    pub const MONO: f32 = 12.0;
}

/// Spacing on the 4 px grid (§4.1).
pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S6: f32 = 24.0;
}

/// Icon size in text and on chevrons (§7: 16 inline, 20 in bars, 24 in cards, 32 in the empty state).
pub const ICON_INLINE: f32 = 16.0;

/// Radii (§4.3).
pub mod radius {
    /// Checkboxes, key caps, small badges.
    pub const SM: u8 = 4;
    /// Buttons, fields, chips, segments, rows, notices.
    pub const MD: u8 = 6;
    /// Lists, popups, segment groups, cards in the content.
    pub const LG: u8 = 8;
    /// Dialog cards.
    pub const XL: u8 = 12;
    /// Counter badges and status labels.
    pub const PILL: u8 = 255;
}

/// Height of all controls in a row: buttons, fields, combo boxes, icon buttons, list rows.
pub const CONTROL: f32 = 32.0;
/// Compact in bars: chips and segments.
pub const COMPACT: f32 = 24.0;

/// Chosen appearance: like the system, light or dark.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

impl Mode {
    fn preference(self) -> ThemePreference {
        match self {
            Mode::System => ThemePreference::System,
            Mode::Light => ThemePreference::Light,
            Mode::Dark => ThemePreference::Dark,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Mode::System => "system",
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }

    /// Saved choice from `view.toml` next to the configuration (`appearance = "light"`).
    pub fn load(dir: &Path) -> Self {
        let saved = read_view(dir).appearance;
        [Mode::Light, Mode::Dark, Mode::System]
            .into_iter()
            .find(|m| saved.as_deref() == Some(m.key()))
            .unwrap_or_default()
    }

    pub fn save(self, dir: &Path) {
        let mut view = read_view(dir);
        view.appearance = Some(self.key().to_string());
        write_view(dir, &view);
    }
}

const VIEW_FILE: &str = "view.toml";

/// `view.toml`: appearance, language and whether the document list is visible. These take effect at once and are not
/// part of the configuration.
#[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct View {
    pub appearance: Option<String>,
    pub language: Option<String>,
    /// `false`: the document list on the left is hidden (R11: the state of the bars is kept).
    pub list: Option<bool>,
}

pub fn read_view(dir: &Path) -> View {
    std::fs::read_to_string(dir.join(VIEW_FILE)).ok().and_then(|raw| toml::from_str(&raw).ok()).unwrap_or_default()
}

pub fn write_view(dir: &Path, view: &View) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(raw) = toml::to_string(view) {
        let _ = std::fs::write(dir.join(VIEW_FILE), raw);
    }
}

/// Sets fonts, spacing and both color sets and selects the appearance.
pub fn apply(ctx: &egui::Context, mode: Mode) {
    ctx.set_visuals_of(Theme::Dark, visuals(&tokens::DARK, true));
    ctx.set_visuals_of(Theme::Light, visuals(&tokens::LIGHT, false));
    set_mode(ctx, mode);
    {
        ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = egui::vec2(space::S2, space::S2);
            s.spacing.button_padding = egui::vec2(space::S3, space::S1);
            s.spacing.interact_size.y = CONTROL;
            s.spacing.icon_spacing = space::S2;
            s.spacing.menu_margin = Margin::same(space::S1 as i8);
            // No fading at the edges of scroll areas: faded text would look disabled (R2).
            s.spacing.scroll.fade.strength = 0.0;
            for (style, size) in [
                (egui::TextStyle::Body, text::BODY),
                (egui::TextStyle::Button, text::BODY),
                (egui::TextStyle::Heading, text::VALUE),
                (egui::TextStyle::Small, text::SMALL),
                (egui::TextStyle::Monospace, text::MONO),
            ] {
                if let Some(font) = s.text_styles.get_mut(&style) {
                    font.size = size;
                }
            }
        });
    }
    install_fonts(ctx);
}

/// Font family for semibold labels (overlines, table header, titles).
pub fn semibold() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

/// Overline (`overline`): field and section label, 11 px, semibold, capitals, letter-spaced, muted.
pub fn field_label(label: &str) -> egui::RichText {
    egui::RichText::new(label.to_uppercase())
        .size(text::OVERLINE)
        .family(semibold())
        .extra_letter_spacing(0.5)
        .color(tokens::muted())
}

/// Column header of a table (`head`): 12 px, semibold, muted.
pub fn head_text(label: &str) -> egui::RichText {
    egui::RichText::new(label).size(text::SMALL).family(semibold()).color(tokens::muted())
}

pub fn set_mode(ctx: &egui::Context, mode: Mode) {
    ctx.set_theme(mode.preference());
    sync(ctx);
}

/// At the start of every frame: the token functions return the colors of the appearance egui is showing.
pub fn sync(ctx: &egui::Context) {
    tokens::set_light(ctx.theme() == Theme::Light);
}

/// Defaults for all egui widgets. egui shares the border and fill of a state between button, combo box, checkbox and
/// field; so the border rule for controls with content applies here (at most `line`), and fields, checkboxes and radio
/// buttons get `line_strong` in their own widgets.
fn visuals(p: &tokens::Palette, dark: bool) -> Visuals {
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface_raised;
    visuals.window_stroke = Stroke::new(1.0_f32, p.line);
    visuals.window_corner_radius = CornerRadius::same(radius::XL);
    visuals.menu_corner_radius = CornerRadius::same(radius::LG);
    visuals.window_shadow = p.shadow;
    visuals.popup_shadow = p.shadow;
    // Input fields: recessed below the surface in dark mode, white in light mode.
    visuals.extreme_bg_color = if dark { p.bg } else { p.surface };
    visuals.faint_bg_color = p.surface_muted;
    visuals.code_bg_color = p.surface_muted;
    // No override_text_color: otherwise placeholders in input fields look like real values.
    visuals.hyperlink_color = p.accent;
    visuals.selection.bg_fill = p.selection;
    // Border of a focused field; selected list rows set their text themselves (`list_style`).
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    visuals.warn_fg_color = p.warn;
    visuals.error_fg_color = p.error;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = p.surface;
    widgets.noninteractive.weak_bg_fill = p.surface;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
    widgets.noninteractive.fg_stroke.color = p.text;
    widgets.noninteractive.corner_radius = CornerRadius::same(radius::MD);
    for (state, fill, stroke) in [
        (&mut widgets.inactive, p.surface_muted, p.line),
        (&mut widgets.hovered, p.hover, p.line),
        (&mut widgets.active, p.surface_pressed, p.line),
        (&mut widgets.open, p.accent_subtle, p.accent),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0_f32, stroke);
        state.corner_radius = CornerRadius::same(radius::MD);
        state.fg_stroke.color = p.text;
        // Nothing grows under the pointer (R1).
        state.expansion = 0.0;
    }
    visuals
}

/// Group heading: overline with space above.
pub fn section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(space::S3);
    ui.label(field_label(label));
    ui.add_space(space::S1);
}

/// Field with its label as an overline above (§8 "Field"): the one form for all fields of the settings.
pub fn labeled<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.add_space(space::S1);
        let gap = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = space::S1;
        ui.label(field_label(label));
        ui.spacing_mut().item_spacing.y = gap;
        add(ui)
    })
    .inner
}

/// Bars and panels (status bar, queue, settings tabs): flush with the window edge, separated from the content by a
/// line, without shadow.
pub fn bar_frame(style: &egui::Style) -> egui::Frame {
    egui::Frame::side_top_panel(style).fill(tokens::surface()).inner_margin(Margin::symmetric(12, 8))
}

/// Status bar at the bottom: lower than the other bars.
pub fn status_frame(style: &egui::Style) -> egui::Frame {
    egui::Frame::side_top_panel(style).fill(tokens::surface()).inner_margin(Margin::symmetric(12, 4))
}

/// Area of the content view (document and form, settings).
pub fn canvas_frame(style: &egui::Style) -> egui::Frame {
    egui::Frame::central_panel(style).fill(tokens::bg()).inner_margin(Margin::same(space::S4 as i8))
}

/// Card in the content (empty state): `surface_raised` with border and shadow.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(tokens::surface_raised())
        .stroke(Stroke::new(1.0_f32, tokens::line()))
        .corner_radius(radius::XL)
        .shadow(tokens::shadow())
        .inner_margin(Margin::same(space::S6 as i8))
}

/// Dialog card over dimmed content: `surface_raised`, radius 12, border, shadow, padding 24.
pub fn dialog(id: egui::Id) -> egui::Modal {
    egui::Modal::new(id).backdrop_color(tokens::scrim()).frame(card_frame())
}

/// Enter confirms a dialog card (R4): `true` if Enter was pressed without Shift. The key is consumed before the card is
/// drawn, so neither a field behind it nor a focused button triggers again. Only for cards without their own text
/// field.
pub fn dialog_enter(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
}

/// Progress as a pill with a bar (§8, R6): bottom centre above the content, 48 high, at least 300 wide,
/// `surface_raised` with an accent border, below it a 4 px track. `slot` stacks several pills from bottom to top. The
/// text says what is running and how far it is.
pub fn progress_pill(ctx: &egui::Context, slot: usize, text: &str, share: f32) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique(("progress", slot))));
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(text::BODY), tokens::fg());
    let width = (galley.size().x + 2.0 * space::S4).max(300.0);
    let screen = ctx.content_rect();
    // Above the status bar (40) with a gap; each further pill 56 higher.
    let centre = egui::pos2(screen.center().x, screen.bottom() - 40.0 - space::S4 - 24.0 - 56.0 * slot as f32);
    let pill = egui::Rect::from_center_size(centre, egui::vec2(width, 48.0));
    painter.add(tokens::shadow().as_shape(pill, radius::LG));
    painter.rect(
        pill,
        radius::LG,
        tokens::surface_raised(),
        Stroke::new(1.0_f32, tokens::accent()),
        egui::StrokeKind::Inside,
    );
    painter.galley(egui::pos2(pill.center().x - galley.size().x / 2.0, pill.top() + 9.0), galley, tokens::fg());
    let track = egui::Rect::from_min_size(
        egui::pos2(pill.left() + space::S4, pill.bottom() - 14.0),
        egui::vec2(pill.width() - 2.0 * space::S4, 4.0),
    );
    painter.rect_filled(track, radius::PILL, tokens::line());
    let done = egui::Rect::from_min_size(track.min, egui::vec2(track.width() * share.clamp(0.0, 1.0), track.height()));
    painter.rect_filled(done, radius::PILL, tokens::accent());
}

/// Title of a dialog card (`value`).
pub fn dialog_title(label: &str) -> egui::RichText {
    egui::RichText::new(label).size(text::VALUE).family(semibold())
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonKind {
    Primary,
    Secondary,
    Danger,
}

/// Button after §8. The fill depends on the state (rest, hovered, pressed); egui knows only one fill for a button with
/// its own color, so the widget paints it behind the label itself.
pub struct Button {
    label: String,
    kind: ButtonKind,
    min_size: egui::Vec2,
}

impl Button {
    pub fn min_size(mut self, size: egui::Vec2) -> Self {
        self.min_size = size;
        self
    }
}

impl egui::Widget for Button {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let p = tokens::palette();
        let behind = ui.painter().add(egui::Shape::Noop);
        let filled = self.kind != ButtonKind::Secondary;
        let color = if filled { Color32::WHITE } else { p.text };
        let min_w = if self.kind == ButtonKind::Primary { 120.0 } else { 0.0 };
        let size = self.min_size.max(egui::vec2(min_w, CONTROL));
        let button = egui::Button::new(egui::RichText::new(self.label).size(text::BODY).color(color))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(radius::MD)
            .min_size(size);
        let resp = ui.add(button);
        let live = ui.is_enabled();
        let hovered = live && resp.hovered();
        let pressed = live && resp.is_pointer_button_down_on();
        let (fill, stroke) = match self.kind {
            ButtonKind::Primary => (if hovered { p.accent_fill_hover } else { p.accent_fill }, Stroke::NONE),
            ButtonKind::Danger => (if hovered { p.danger_fill_hover } else { p.danger_fill }, Stroke::NONE),
            ButtonKind::Secondary => {
                let fill = match (pressed, hovered) {
                    (true, _) => p.surface_pressed,
                    (false, true) => p.hover,
                    _ => p.surface_muted,
                };
                (fill, Stroke::new(1.0_f32, if hovered { p.accent } else { p.line }))
            }
        };
        let shape = egui::epaint::RectShape::new(resp.rect, radius::MD, fill, stroke, egui::StrokeKind::Inside);
        ui.painter().set(behind, shape);
        if resp.has_focus() {
            focus_ring(ui, resp.rect, radius::MD);
        }
        resp
    }
}

/// The one main action of a view: white on `accent_fill`, 32 px high, at least 120 wide.
pub fn primary_button(label: impl Into<String>) -> Button {
    Button { label: label.into(), kind: ButtonKind::Primary, min_size: egui::Vec2::ZERO }
}

/// Secondary action: `surface_muted` with a fine border; accent border under the pointer.
pub fn secondary_button(label: impl Into<String>) -> Button {
    Button { label: label.into(), kind: ButtonKind::Secondary, min_size: egui::Vec2::ZERO }
}

/// Irreversible action (delete): white on `danger_fill`.
pub fn danger_button(label: impl Into<String>) -> Button {
    Button { label: label.into(), kind: ButtonKind::Danger, min_size: egui::Vec2::ZERO }
}

/// Unobtrusive action (ghost): text in the accent, fill only under the pointer (neutral), 32 px high.
pub fn ghost_button(ui: &mut egui::Ui, label: &str) -> Response {
    let behind = ui.painter().add(egui::Shape::Noop);
    let color = if ui.is_enabled() { tokens::accent() } else { tokens::disabled() };
    let button = egui::Button::new(egui::RichText::new(label).size(text::BODY).color(color))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .corner_radius(radius::MD)
        .min_size(egui::vec2(0.0, CONTROL));
    let resp = ui.add(button);
    if resp.hovered() && ui.is_enabled() {
        ui.painter().set(behind, egui::Shape::rect_filled(resp.rect, radius::MD, tokens::hover()));
    }
    resp
}

/// Clickable chip (name parts, folder parts): `surface_muted`, fine border, radius 6.
pub fn chip_button(text: impl Into<egui::WidgetText>, height: f32) -> egui::Button<'static> {
    egui::Button::new(text)
        .corner_radius(radius::MD)
        .stroke(Stroke::new(1.0_f32, tokens::line()))
        .min_size(egui::vec2(0.0, height))
}

/// Keyboard focus ring: 2 px `accent_strong` with a 1 px gap.
pub fn focus_ring(ui: &egui::Ui, rect: egui::Rect, corner: u8) {
    let ring = rect.expand(3.0);
    let stroke = Stroke::new(2.0_f32, tokens::accent_strong());
    ui.painter().rect_stroke(ring, corner.saturating_add(3), stroke, egui::StrokeKind::Inside);
}

/// Padding of input fields; the width of a field (`desired_width`) includes it.
pub const FIELD_MARGIN: f32 = space::S2;

/// Input field: 32 px high, recessed, border `line_strong` (border rule), when focused an accent border and ring.
pub fn field(ui: &mut egui::Ui, edit: egui::TextEdit) -> Response {
    ui.scope(|ui| {
        let strong = Stroke::new(1.0_f32, tokens::line_strong());
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_stroke = strong;
        widgets.hovered.bg_stroke = strong;
        let edit = edit.margin(Margin::symmetric(FIELD_MARGIN as i8, 4)).min_size(egui::vec2(0.0, CONTROL));
        let resp = ui.add(edit.align(egui::Align2::LEFT_CENTER));
        if resp.has_focus() {
            focus_ring(ui, resp.rect, radius::MD);
        }
        resp
    })
    .inner
}

/// Multi-line input field: like `field`, but the text starts at the top.
pub fn text_area(ui: &mut egui::Ui, edit: egui::TextEdit) -> Response {
    ui.scope(|ui| {
        let strong = Stroke::new(1.0_f32, tokens::line_strong());
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_stroke = strong;
        widgets.hovered.bg_stroke = strong;
        let resp = ui.add(edit.margin(Margin::symmetric(FIELD_MARGIN as i8, 6)));
        if resp.has_focus() {
            focus_ring(ui, resp.rect, radius::MD);
        }
        resp
    })
    .inner
}

/// Status label: pill with a dot and text in the status color on its background. The accent sits on `accent_subtle`;
/// other colors (source of a suggestion) get a calm fill with a fine border. Only status colors have a dot. `min_width`
/// keeps the width fixed when the text changes (R1). Painted instead of laid out: this way the dot also sits before the
/// text in a right-to-left row.
pub fn chip_min(ui: &mut egui::Ui, label: &str, color: Color32, min_width: f32) -> Response {
    let status = match color {
        c if c == tokens::status_ok() => Some(tokens::status_ok_bg()),
        c if c == tokens::status_err() => Some(tokens::status_err_bg()),
        c if c == tokens::status_warn() => Some(tokens::status_warn_bg()),
        _ => None,
    };
    // Accent (e.g. "recommended"): on `accent_subtle` only `accent_strong`, otherwise the contrast is too low.
    let (fill, stroke, color) = match status {
        Some(bg) => (bg, Stroke::NONE, color),
        None if color == tokens::accent() => (tokens::accent_subtle(), Stroke::NONE, tokens::accent_strong()),
        None => (tokens::surface_muted(), Stroke::new(1.0_f32, tokens::line()), color),
    };
    let galley = chip_galley(ui, label);
    let dot = if status.is_some() { CHIP_DOT } else { 0.0 };
    let pad = egui::vec2(space::S2, 2.0);
    let inner = egui::vec2((galley.size().x + dot).max(min_width), galley.size().y);
    let (rect, resp) = ui.allocate_exact_size(inner + 2.0 * pad, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, radius::PILL, fill, stroke, egui::StrokeKind::Inside);
    let left = rect.left() + pad.x;
    if status.is_some() {
        painter.circle_filled(egui::pos2(left + 3.0, rect.center().y), 3.0, color);
    }
    painter.galley(egui::pos2(left + dot, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Space for the dot of a status label including the gap to the text.
const CHIP_DOT: f32 = 6.0 + space::S1 + 2.0;

fn chip_galley(ui: &egui::Ui, label: &str) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(egui::RichText::new(label).size(text::SMALL)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    )
}

pub fn chip(ui: &mut egui::Ui, label: &str, color: Color32) -> Response {
    chip_min(ui, label, color, 0.0)
}

/// Width `chip_min` needs so that each of the texts fits.
pub fn chip_width(ui: &egui::Ui, labels: &[&str], status: bool) -> f32 {
    let dot = if status { CHIP_DOT } else { 0.0 };
    let widest = labels.iter().map(|l| chip_galley(ui, l).size().x).fold(0.0, f32::max);
    (widest + dot).ceil()
}

/// Painted status dot (the built-in fonts contain no "●").
pub fn status_dot(ui: &mut egui::Ui, color: Color32, filled: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
    let center = rect.center();
    if filled {
        ui.painter().circle_filled(center, 4.5, color);
    } else {
        ui.painter().circle_stroke(center, 4.0, Stroke::new(1.5_f32, color));
    }
}

/// Folder icon (Lucide "folder").
pub fn folder_icon(ui: &mut egui::Ui, color: Color32) {
    ui.label(icon_text(icon::FOLDER, 16.0, color));
}

/// File icon (Lucide "file-text").
pub fn file_icon(ui: &mut egui::Ui, color: Color32) {
    ui.label(icon_text(icon::FILE_TEXT, 16.0, color));
}

/// Side length of icon buttons; rows that hold them compute their space with it.
pub const ARROW_BUTTON: f32 = CONTROL;

/// Small button with an arrow (Lucide chevron), 32 × 32 px like all click areas.
pub fn arrow_button(ui: &mut egui::Ui, dir: Arrow, enabled: bool, hover: &str) -> bool {
    let glyph = match dir {
        Arrow::Up => icon::CHEVRON_UP,
        Arrow::Down => icon::CHEVRON_DOWN,
        Arrow::Left => icon::CHEVRON_LEFT,
        Arrow::Right => icon::CHEVRON_RIGHT,
    };
    icon_button(ui, glyph, enabled, hover)
}

/// Icon-only button, 32 × 32 px: muted, under the pointer `hover` and the icon in `accent_strong`; disabled in
/// `disabled`. Takes no keyboard focus (R7), otherwise the space bar would trigger it again.
pub fn icon_button(ui: &mut egui::Ui, glyph: char, enabled: bool, hover: &str) -> bool {
    let size = egui::vec2(ARROW_BUTTON, ARROW_BUTTON);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::CLICK } else { egui::Sense::hover() });
    paint_icon_button(ui, rect, &resp, glyph, 16.0, enabled);
    let resp = resp.on_hover_text(hover);
    enabled && resp.clicked()
}

/// Paints an icon button in `rect` (also for buttons that allocate their area themselves).
pub fn paint_icon_button(ui: &egui::Ui, rect: egui::Rect, resp: &Response, glyph: char, size: f32, enabled: bool) {
    let hovered = enabled && resp.hovered();
    if hovered {
        let fill = if resp.is_pointer_button_down_on() { tokens::palette().surface_pressed } else { tokens::hover() };
        ui.painter().rect_filled(rect, radius::MD, fill);
    }
    let color = match (enabled, hovered) {
        (false, _) => tokens::disabled(),
        (true, true) => tokens::accent_strong(),
        (true, false) => tokens::muted(),
    };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(size), color);
}

/// Collapsing header with a Lucide chevron instead of the egui triangle.
pub fn collapsing(text: impl Into<egui::WidgetText>) -> egui::CollapsingHeader {
    egui::CollapsingHeader::new(text).icon(|ui, openness, resp| {
        let glyph = if openness > 0.5 { icon::CHEVRON_DOWN } else { icon::CHEVRON_RIGHT };
        let color = if resp.hovered() { tokens::accent_strong() } else { tokens::muted() };
        let font = egui::FontId::proportional(ICON_INLINE);
        ui.painter().text(resp.rect.center(), egui::Align2::CENTER_CENTER, glyph, font, color);
    })
}

/// Rows of a list or menu (popup): without borders; the highlighted and the selected row on `accent_subtle` with text
/// in `text`. Call at the start of every popup; egui gives popups the style of the window, not that of the calling
/// place.
pub fn list_style(ui: &mut egui::Ui) {
    let p = tokens::palette();
    let visuals = ui.visuals_mut();
    visuals.selection.bg_fill = p.accent_subtle;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.text);
    let widgets = &mut visuals.widgets;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
        state.bg_stroke = Stroke::NONE;
    }
    widgets.hovered.weak_bg_fill = p.accent_subtle;
    widgets.hovered.bg_fill = p.accent_subtle;
    widgets.active.weak_bg_fill = p.accent_subtle;
}

/// Combo box with a Lucide chevron instead of the filled egui triangle; looks like a secondary button.
pub fn combo<'a>(id: impl egui::AsIdSalt) -> Combo<'a> {
    Combo(egui::ComboBox::from_id_salt(id).icon(|ui, rect, visuals, _open| {
        let font = egui::FontId::proportional(ICON_INLINE);
        let color = visuals.fg_stroke.color;
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon::CHEVRON_DOWN, font, color);
    }))
}

/// `egui::ComboBox` with a border after the border rule and rows after `list_style`.
pub struct Combo<'a>(egui::ComboBox<'a>);

impl Combo<'_> {
    pub fn selected_text(self, text: impl Into<egui::WidgetText>) -> Self {
        Self(self.0.selected_text(text))
    }

    pub fn width(self, width: f32) -> Self {
        Self(self.0.width(width))
    }

    pub fn height(self, height: f32) -> Self {
        Self(self.0.height(height))
    }

    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        ui.scope(|ui| {
            ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.0_f32, tokens::accent());
            self.0.show_ui(ui, |ui| {
                list_style(ui);
                contents(ui)
            })
        })
        .inner
    }
}

/// Segmented control: a group with a shared `line` border (radius 8, 2 px inside); the segments have no border of their
/// own. `height` is the height of the whole group (32 in a row with other controls).
pub fn segments<R>(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, tokens::line()))
        .corner_radius(radius::LG)
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().interact_size.y = height - 6.0;
            ui.horizontal(add).inner
        })
        .inner
}

/// A segment in `segments`: selected `accent_subtle` with an `accent` border and content in `accent_strong`, `hover`
/// under the pointer. The width depends only on the text, not on the state (R1).
pub fn segment(ui: &mut egui::Ui, selected: bool, label: impl Into<egui::WidgetText>) -> Response {
    let galley = label.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    let size = egui::vec2(galley.size().x + 2.0 * space::S3, ui.spacing().interact_size.y);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::CLICK);
    let p = tokens::palette();
    let live = ui.is_enabled();
    if selected {
        ui.painter().rect(rect, radius::MD, p.accent_subtle, Stroke::new(1.0_f32, p.accent), egui::StrokeKind::Inside);
    } else if live && resp.hovered() {
        ui.painter().rect_filled(rect, radius::MD, p.hover);
    }
    let color = if selected { p.accent_strong } else { p.text };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// Filter chip to switch on and off (several at once): 24 high, border `line`; on with `accent_subtle` and border
/// `accent`.
pub fn filter_chip(ui: &mut egui::Ui, on: bool, label: impl Into<egui::WidgetText>) -> Response {
    let galley = label.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    let size = egui::vec2(galley.size().x + 2.0 * space::S3, COMPACT);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::CLICK);
    let p = tokens::palette();
    let (fill, border) = match (on, ui.is_enabled() && resp.hovered()) {
        (true, _) => (p.accent_subtle, p.accent),
        (false, true) => (p.hover, p.line),
        (false, false) => (p.surface_muted, p.line),
    };
    ui.painter().rect(rect, radius::MD, fill, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.text);
    resp
}

/// `segment` for a value: a click selects `value`.
pub fn segment_value<V: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut V,
    value: V,
    label: impl Into<egui::WidgetText>,
) -> Response {
    let mut resp = segment(ui, *current == value, label);
    if resp.clicked() && *current != value {
        *current = value;
        resp.mark_changed();
    }
    resp
}

/// Entry of a vertical tab list (settings): 32 high across the full width; selected with `accent_subtle`, a 2 px accent
/// line on the left and semibold text, otherwise muted.
pub fn nav_item(ui: &mut egui::Ui, selected: bool, label: &str, size: f32) -> Response {
    let p = tokens::palette();
    let family = if selected { semibold() } else { egui::FontFamily::Proportional };
    let color = if selected { p.text } else { p.muted };
    let galley = egui::WidgetText::from(egui::RichText::new(label).size(size).family(family)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        ui.available_width() - 2.0 * space::S3,
        egui::TextStyle::Button,
    );
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), CONTROL), egui::Sense::CLICK);
    if selected {
        ui.painter().rect_filled(rect, radius::MD, p.accent_subtle);
        let bar = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 6.0), egui::vec2(2.0, rect.height() - 12.0));
        ui.painter().rect_filled(bar, 1.0, p.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, radius::MD, p.hover);
    }
    let pos = egui::pos2(rect.left() + space::S3, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
    resp
}

/// Checkbox after §8: 14 × 14, radius 4, border `line_strong`; on: border `accent`, check mark `accent_strong`.
pub struct Check<'a> {
    value: &'a mut bool,
    label: egui::WidgetText,
}

pub fn check(value: &mut bool, label: impl Into<egui::WidgetText>) -> Check<'_> {
    Check { value, label: label.into() }
}

/// `ui.checkbox` with the design system's checkbox.
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    ui.add(check(value, label))
}

impl egui::Widget for Check<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let Check { value, label } = self;
        let mut resp = toggle_mark(ui, *value, label, false);
        if resp.clicked() {
            *value = !*value;
            resp.mark_changed();
        }
        resp
    }
}

/// Radio button after §8 (round, otherwise like the checkbox).
pub fn radio_value<V: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut V,
    value: V,
    label: impl Into<egui::WidgetText>,
) -> Response {
    let on = *current == value;
    let mut resp = toggle_mark(ui, on, label.into(), true);
    if resp.clicked() && !on {
        *current = value;
        resp.mark_changed();
    }
    resp
}

/// Checkbox or radio button with a label; the click area covers both, at least 32 px high.
fn toggle_mark(ui: &mut egui::Ui, on: bool, label: egui::WidgetText, round: bool) -> Response {
    const BOX: f32 = 14.0;
    let gap = ui.spacing().icon_spacing;
    let galley = (!label.is_empty()).then(|| {
        let wrap = ui.available_width() - BOX - gap;
        label.into_galley(ui, None, wrap, egui::TextStyle::Button)
    });
    let text_w = galley.as_ref().map_or(0.0, |g| gap + g.size().x);
    let text_h = galley.as_ref().map_or(0.0, |g| g.size().y);
    let size = egui::vec2(BOX + text_w, CONTROL.max(text_h));
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let kind = if round { egui::Role::RadioButton } else { egui::Role::CheckBox };
    let text = galley.as_ref().map_or(String::new(), |g| g.text().to_string());
    resp.widget_info(|| egui::WidgetInfo::selected(kind, ui.is_enabled(), on, &text));
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = tokens::palette();
    let live = ui.is_enabled();
    let mark =
        egui::Rect::from_center_size(egui::pos2(rect.left() + BOX / 2.0, rect.center().y), egui::Vec2::splat(BOX));
    let fill = if live && resp.hovered() { p.hover } else { ui.visuals().extreme_bg_color };
    let border = Stroke::new(1.0_f32, if on { p.accent } else { p.line_strong });
    let painter = ui.painter();
    if round {
        painter.circle(mark.center(), BOX / 2.0, fill, border);
        if on {
            painter.circle_filled(mark.center(), 3.5, p.accent_strong);
        }
    } else {
        painter.rect(mark, radius::SM, fill, border, egui::StrokeKind::Inside);
        if on {
            let m = mark.shrink(3.0);
            let points = vec![
                egui::pos2(m.left(), m.center().y),
                egui::pos2(m.left() + m.width() * 0.4, m.bottom() - 1.0),
                egui::pos2(m.right(), m.top() + 1.0),
            ];
            painter.add(egui::Shape::line(points, Stroke::new(1.75_f32, p.accent_strong)));
        }
    }
    if resp.has_focus() {
        focus_ring(ui, mark, if round { 7 } else { radius::SM });
    }
    if let Some(galley) = galley {
        let pos = egui::pos2(mark.right() + gap, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, p.text);
    }
    resp
}

#[derive(Clone, Copy)]
pub enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

/// Segoe UI (regular and semibold), Consolas for monospace; otherwise the usual system fonts. The built-in egui fonts
/// remain the fallback for missing characters.
fn install_fonts(ctx: &egui::Context) {
    type Paths = Vec<std::path::PathBuf>;
    let (regular, bold, mono): (Paths, Paths, Paths) = if cfg!(windows) {
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
        let fonts = std::path::Path::new(&windir).join("Fonts");
        (
            vec![fonts.join("segoeui.ttf")],
            vec![fonts.join("seguisb.ttf"), fonts.join("segoeuib.ttf")],
            vec![fonts.join("consola.ttf")],
        )
    } else {
        let dirs = [
            "/usr/share/fonts/truetype/noto/",
            "/usr/share/fonts/noto/",
            "/usr/share/fonts/google-noto/",
            "/usr/share/fonts/truetype/dejavu/",
            "/usr/share/fonts/TTF/",
            "/usr/share/fonts/dejavu-sans-fonts/",
            "/usr/share/fonts/dejavu-sans-mono-fonts/",
        ];
        let with = |names: &[&str]| -> Paths {
            names.iter().flat_map(|n| dirs.iter().map(move |d| std::path::Path::new(d).join(n))).collect()
        };
        (
            with(&["NotoSans-Regular.ttf", "DejaVuSans.ttf"]),
            with(&["NotoSans-SemiBold.ttf", "DejaVuSans-Bold.ttf", "NotoSans-Bold.ttf"]),
            with(&["DejaVuSansMono.ttf"]),
        )
    };
    let read = |paths: Paths| paths.into_iter().find_map(|p| std::fs::read(p).ok());
    let mut defs = egui::FontDefinitions::default();
    let proportional = egui::FontFamily::Proportional;
    if let Some(bytes) = read(regular) {
        defs.font_data.insert("system-ui".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.entry(proportional.clone()).or_default().insert(0, "system-ui".into());
    }
    let mut bold_family = defs.families.get(&proportional).cloned().unwrap_or_default();
    if let Some(bytes) = read(bold) {
        defs.font_data.insert("system-ui-semibold".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        bold_family.insert(0, "system-ui-semibold".into());
    }
    defs.families.insert(semibold(), bold_family);
    if let Some(bytes) = read(mono) {
        // Consolas has more space above than below; set slightly lower it sits centred in fields and chips.
        let shift = if cfg!(windows) { 0.1 } else { 0.0 };
        let font =
            egui::FontData::from_owned(bytes).tweak(egui::FontTweak { y_offset_factor: shift, ..Default::default() });
        defs.font_data.insert("system-mono".into(), std::sync::Arc::new(font));
        defs.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "system-mono".into());
    }
    // Icons in second place: the first font sets line height and baseline, the icon font only has characters in the
    // private use area and must come before the egui fallback fonts. Slightly larger than the cap height and centred to
    // the font (the icon grid reaches down to the baseline).
    let icons = egui::FontData::from_static(include_bytes!("../assets/lucide/lucide-subset.ttf"))
        .tweak(egui::FontTweak { scale: 1.15, y_offset_factor: 0.04, ..Default::default() });
    defs.font_data.insert("lucide".into(), std::sync::Arc::new(icons));
    for family in [proportional, semibold()] {
        let list = defs.families.entry(family).or_default();
        list.insert(list.len().min(1), "lucide".into());
    }
    ctx.set_fonts(defs);
}
