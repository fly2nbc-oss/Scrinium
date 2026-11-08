//! *General* tab of the settings: about Scrinium, language, appearance, updates and keyboard shortcuts. Everything
//! here takes effect at once and is not part of the configuration. Also the question on first start whether
//! Scrinium may check for updates, and the *Scan* tab.

use eframe::egui::{self, Color32, Id, RichText, Vec2};

use crate::config::{ScanConfig, ScanPreset};
use crate::i18n::{self, Lang, general};
use crate::theme::{self, Mode, text, tokens};
use crate::update::{REPOSITORY, Updates};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeneralAction {
    SetLanguage(Lang),
    SetMode(Mode),
    CheckNow,
    SetUpdates(bool),
}

/// Gap between the sections; `theme::section` adds some space above on its own. Kept small so that the tab fits
/// without scrolling at a normal window size.
const GAP: f32 = 4.0;

/// Keyboard shortcuts in the main window and in the settings, with the key names of the current language.
fn keys() -> [(&'static str, Vec<(String, &'static str)>); 2] {
    let t = general::t();
    let ctrl = |key: &str| format!("{}+{key}", t.key_ctrl);
    [
        (
            t.keys_documents,
            vec![
                (ctrl(t.key_enter), t.do_file),
                (ctrl(&format!("{}+{}", t.key_shift, t.key_enter)), t.do_later),
                (ctrl("Z"), t.do_undo),
                (format!("{} / {}", t.key_page_up, t.key_page_down), t.do_pages),
                (ctrl(&format!("{} / {}", t.key_page_up, t.key_page_down)), t.do_documents),
                (ctrl(t.key_wheel), t.do_zoom),
            ],
        ),
        (
            t.keys_settings,
            vec![
                (ctrl("S"), t.do_save),
                (format!("{} / {}", ctrl("Tab"), ctrl(&format!("{}+Tab", t.key_shift))), t.do_tabs),
                ("Esc".to_string(), t.do_back),
            ],
        ),
    ]
}

fn link(label: &str, url: &str) -> egui::Hyperlink {
    egui::Hyperlink::from_label_and_url(RichText::new(label).color(tokens::accent()), url).open_in_new_tab(true)
}

fn paragraph(ui: &mut egui::Ui, s: &str) {
    ui.add(egui::Label::new(RichText::new(s).color(tokens::muted())).wrap());
}

/// The app icon as a texture, loaded once.
fn app_icon(ui: &egui::Ui) -> egui::TextureHandle {
    let id = Id::new("app-icon-texture");
    if let Some(tex) = ui.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return tex;
    }
    let image = match image::load_from_memory(include_bytes!("../assets/icon.png")) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
        }
        Err(_) => egui::ColorImage::new([1, 1], Color32::TRANSPARENT),
    };
    let tex = ui.ctx().load_texture("app-icon", image, egui::TextureOptions::LINEAR);
    ui.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

/// The *General* tab. Returns what the user did.
pub fn general_tab(ui: &mut egui::Ui, mode: Mode, updates: &Updates) -> Option<GeneralAction> {
    // Body text reads well up to about 760 px; the tab does not need the full width.
    ui.scope(|ui| {
        ui.set_max_width(760.0);
        general_content(ui, mode, updates)
    })
    .inner
}

/// The *Scan* tab.
pub fn scan_tab(ui: &mut egui::Ui, scan: &mut ScanConfig) {
    let t = crate::i18n::scan::t();
    ui.scope(|ui| {
        ui.set_max_width(760.0);
        theme::section(ui, t.scanner);
        scan_section(ui, scan);
        ui.add_space(16.0);
        theme::section(ui, t.preparation);
        preparation_section(ui, scan);
    });
}

fn general_content(ui: &mut egui::Ui, mode: Mode, updates: &Updates) -> Option<GeneralAction> {
    let t = general::t();
    let mut action = None;

    // About Scrinium: icon, name and version, one line on what it does, and the project details.
    theme::section(ui, t.about);
    ui.horizontal_top(|ui| {
        ui.add(egui::Image::new(&app_icon(ui)).fit_to_exact_size(Vec2::splat(48.0)).corner_radius(theme::radius::XL));
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scrinium").size(text::LARGE).family(theme::semibold()));
                ui.label(RichText::new((t.version)(env!("CARGO_PKG_VERSION"))).color(tokens::muted()));
            });
            ui.label(RichText::new(t.tagline).color(tokens::muted()));
            ui.horizontal_wrapped(|ui| {
                let muted = |s: &str| RichText::new(s).size(text::SMALL).color(tokens::muted());
                ui.label(muted(&format!("© 2026 {}", env!("CARGO_PKG_AUTHORS"))));
                ui.label(muted("·"));
                ui.label(muted(env!("CARGO_PKG_LICENSE"))).on_hover_text(t.libraries);
                ui.label(muted("·"));
                ui.add(link(REPOSITORY.trim_start_matches("https://"), REPOSITORY));
            });
        });
    });
    ui.add_space(GAP);

    theme::section(ui, t.language);
    theme::segments(ui, theme::CONTROL, |ui| {
        for lang in Lang::ALL {
            let current = i18n::current() == lang;
            if theme::segment(ui, current, lang.name()).clicked() && !current {
                action = Some(GeneralAction::SetLanguage(lang));
            }
        }
    });
    ui.add_space(GAP);

    theme::section(ui, t.look);
    theme::segments(ui, theme::CONTROL, |ui| {
        for (m, icon, label, hint) in [
            (Mode::System, theme::icon::SUN_MOON, t.mode_system, t.mode_system_hint),
            (Mode::Light, theme::icon::SUN, t.mode_light, t.mode_light_hint),
            (Mode::Dark, theme::icon::MOON, t.mode_dark, t.mode_dark_hint),
        ] {
            let resp = theme::segment(ui, mode == m, theme::with_icon(icon, label)).on_hover_text(hint);
            if resp.clicked() && mode != m {
                action = Some(GeneralAction::SetMode(m));
            }
        }
    });
    ui.add_space(GAP);

    // Updates in one row; what gets sent is shown on hover.
    theme::section(ui, t.updates);
    ui.horizontal_wrapped(|ui| {
        let mut on = updates.enabled();
        if theme::checkbox(ui, &mut on, t.updates_daily).on_hover_text(t.updates_privacy).changed() {
            action = Some(GeneralAction::SetUpdates(on));
        }
        ui.add_space(8.0);
        ui.label(RichText::new(updates.status()).color(tokens::muted()));
        ui.add_space(8.0);
        if ui
            .add_enabled(!updates.busy(), theme::secondary_button(t.check_now))
            .on_disabled_hover_text(t.check_busy)
            .clicked()
        {
            action = Some(GeneralAction::CheckNow);
        }
        if let Some(v) = updates.newer() {
            ui.add_space(8.0);
            ui.add(link(&(t.download)(&v.to_string()), &v.page()));
        }
    });
    ui.add_space(GAP);

    // For reference, hence last: both groups side by side.
    theme::section(ui, t.keys);
    ui.horizontal_top(|ui| {
        for (i, (area, rows)) in keys().into_iter().enumerate() {
            if i > 0 {
                ui.add_space(40.0);
            }
            ui.vertical(|ui| {
                ui.label(theme::head_text(area));
                egui::Grid::new(("keys", i)).num_columns(2).spacing([20.0, 4.0]).min_row_height(18.0).show(ui, |ui| {
                    for (key, what) in rows {
                        ui.label(RichText::new(key).monospace().size(text::SMALL));
                        ui.label(what);
                        ui.end_row();
                    }
                });
            });
        }
    });
    action
}

/// Question on first start. `Some(true)` = check, `Some(false)` = don't.
pub fn question(ctx: &egui::Context) -> Option<bool> {
    let t = general::t();
    let mut answer = None;
    let enter = theme::dialog_enter(ctx);
    theme::dialog(Id::new("update-question")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(theme::dialog_title(t.update_ask_title));
        ui.add_space(8.0);
        paragraph(ui, t.update_ask_text);
        ui.add_space(16.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme::primary_button(t.yes)).clicked() || enter {
                answer = Some(true);
            }
            if ui.add(theme::secondary_button(t.no)).clicked() {
                answer = Some(false);
            }
        });
    });
    answer
}

/// Which scanner responds to a click on the *Scan* button; on Linux also resolution and color.
fn scan_section(ui: &mut egui::Ui, scan: &mut ScanConfig) {
    let t = crate::i18n::scan::t();
    let Some(devices) = crate::scan::devices(ui.ctx()) else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(t.searching).color(tokens::muted()));
        });
        return;
    };
    if devices.is_empty() {
        paragraph(ui, if cfg!(windows) { t.none_found_windows } else { t.none_found_linux });
        return;
    }
    paragraph(ui, if cfg!(windows) { t.help_windows } else { t.help_linux });
    ui.add_space(4.0);
    let auto = if cfg!(windows) { t.auto_windows } else { t.auto_linux };
    let shown = match devices.iter().find(|d| d.id == scan.device) {
        Some(d) => d.label.clone(),
        None if scan.device.is_empty() => auto.to_string(),
        None => (t.not_found)(&scan.device),
    };
    // The section heading already says *Scanner*; the field does not need a second label.
    theme::combo("scan-device").selected_text(shown).width(460.0).show_ui(ui, |ui| {
        ui.selectable_value(&mut scan.device, String::new(), auto);
        for d in devices {
            ui.selectable_value(&mut scan.device, d.id.clone(), &d.label);
        }
    });
    theme::labeled(ui, t.save_in, |ui| {
        ui.horizontal(|ui| {
            let default = crate::config::default_scan_folder().display().to_string();
            theme::field(ui, egui::TextEdit::singleline(&mut scan.folder).hint_text(default).desired_width(360.0))
                .on_hover_text(t.save_in_hint);
            if ui.add(theme::secondary_button(t.browse)).clicked()
                && let Some(dir) = rfd::FileDialog::new().set_title(t.folder_title).pick_folder()
            {
                scan.folder = dir.display().to_string();
            }
        });
    });
    if !cfg!(windows) {
        ui.horizontal_top(|ui| {
            theme::labeled(ui, t.resolution, |ui| {
                let dpi = theme::combo("scan-dpi").selected_text(format!("{} dpi", scan.resolution)).width(160.0);
                dpi.show_ui(ui, |ui| {
                    for dpi in [150, 200, 300, 400, 600] {
                        ui.selectable_value(&mut scan.resolution, dpi, format!("{dpi} dpi"));
                    }
                });
            });
            ui.add_space(16.0);
            theme::labeled(ui, t.color, |ui| theme::checkbox(ui, &mut scan.color, t.color_scan));
        });
    }
}

/// What happens to the pages after scanning: color mode and text recognition.
fn preparation_section(ui: &mut egui::Ui, scan: &mut ScanConfig) {
    let t = crate::i18n::scan::t();
    paragraph(ui, t.preparation_help);
    ui.add_space(4.0);
    theme::labeled(ui, t.color_mode, |ui| {
        ui.horizontal(|ui| {
            theme::combo("scan-preset").selected_text(scan.preset.label()).width(200.0).show_ui(ui, |ui| {
                for p in ScanPreset::ALL {
                    ui.selectable_value(&mut scan.preset, p, p.label()).on_hover_text(p.hint());
                }
            });
            ui.label(RichText::new(scan.preset.hint()).color(tokens::muted()));
        });
    });
    theme::labeled(ui, t.text, |ui| theme::checkbox(ui, &mut scan.text_layer, t.text_layer));
}
