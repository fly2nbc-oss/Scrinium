//! Building the file name in the settings: parts as chips to move (drag), remove and add, a selectable
//! separator, and the template as text as well. What gets saved is still the text, such as
//! `{date}_{doctype}_{sender}`; whatever cannot be split into parts stays editable as text.

use eframe::egui::{self, RichText, Stroke, TextEdit, Vec2};

use crate::form::text_width;
use crate::i18n::settings::t;
use crate::theme::{self, text, tokens};

/// Parts; `field_label` provides their names in the UI. `{title}` counts as the subject.
const FIELDS: [&str; 9] = ["date", "doctype", "sender", "subject", "person", "extra", "year", "month", "day"];

/// Name of a part in the UI; `None` for unknown ones.
fn field_label(key: &str) -> Option<&'static str> {
    let t = t();
    Some(match key {
        "date" => t.part_date,
        "doctype" => t.part_doctype,
        "sender" => t.part_sender,
        "subject" | "title" => t.part_subject,
        "person" => t.part_person,
        "extra" => t.part_extra,
        "year" => t.part_year,
        "month" => t.part_month,
        "day" => t.part_day,
        _ => return None,
    })
}

/// Separators between the parts; the first one is the default.
const SEPARATORS: [&str; 3] = ["_", "-", " "];

fn separator_label(sep: &str) -> &'static str {
    let t = t();
    match sep {
        "_" => t.sep_underscore,
        "-" => t.sep_hyphen,
        _ => t.sep_space,
    }
}

/// Templates for a quick start; their names are in the same order in `presets` of the texts.
pub const PRESETS: [&str; 4] = [
    "{date}_{doctype}_{sender}_{subject}_{person}_{extra}",
    "{date}_{doctype}_{sender}_{subject}",
    "{date}_{sender}_{doctype}",
    "{sender}_{doctype}_{date}",
];

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Field(String),
    Text(String),
}

impl Block {
    fn label(&self) -> String {
        match self {
            Block::Field(f) => field_label(f).map_or_else(|| f.clone(), str::to_string),
            Block::Text(s) => (t().quote)(s),
        }
    }
}

fn block(part: &str) -> Option<Block> {
    if part.is_empty() {
        return None;
    }
    if let Some(name) = part.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let known = name == "title" || FIELDS.contains(&name);
        return known.then(|| Block::Field(name.to_string()));
    }
    (!part.contains(['{', '}'])).then(|| Block::Text(part.to_string()))
}

/// Splits a template into parts and a separator. `None` if it is more than parts with one
/// separator in between (e.g. `{year}-{month}` in one piece); then it stays editable as text.
pub fn parse(template: &str) -> Option<(Vec<Block>, &'static str)> {
    if template.trim().is_empty() {
        return Some((Vec::new(), SEPARATORS[0]));
    }
    SEPARATORS.iter().find_map(|sep| {
        let blocks: Option<Vec<Block>> = template.split(sep).map(block).collect();
        blocks.filter(|b| b.iter().any(|x| matches!(x, Block::Field(_)))).map(|b| (b, *sep))
    })
}

pub fn compose(blocks: &[Block], sep: &str) -> String {
    blocks
        .iter()
        .map(|b| match b {
            Block::Field(f) => format!("{{{f}}}"),
            Block::Text(t) => t.clone(),
        })
        .collect::<Vec<_>>()
        .join(sep)
}

/// Moves the part `from` to position `to` (before the part that stood there).
fn move_block(blocks: &mut Vec<Block>, from: usize, to: usize) {
    if from >= blocks.len() {
        return;
    }
    let b = blocks.remove(from);
    let to = if to > from { to - 1 } else { to }.min(blocks.len());
    blocks.insert(to, b);
}

/// Parts as chips: dragging moves a part, the cross removes it, the add button adds one.
pub fn blocks(ui: &mut egui::Ui, template: &mut String) {
    let Some((mut blocks, sep)) = parse(template) else {
        ui.label(RichText::new(t().own_template).size(text::SMALL).color(tokens::muted()));
        return;
    };
    if chips(ui, &mut blocks) {
        *template = compose(&blocks, sep);
    }
}

fn chips(ui: &mut egui::Ui, blocks: &mut Vec<Block>) -> bool {
    let t = t();
    let mut changed = false;
    let mut moved: Option<(usize, usize)> = None;
    let mut removed: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, b) in blocks.iter().enumerate() {
            let color = if matches!(b, Block::Text(_)) { tokens::muted() } else { tokens::fg() };
            let label = RichText::new(b.label()).color(color);
            // Part and cross stay together: if they no longer fit in the row, a new row starts.
            if text_width(ui, label.clone()) + 20.0 + 20.0 > ui.available_size_before_wrap().x {
                ui.end_row();
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let chip = ui.dnd_drag_source(egui::Id::unique(("name-block", i)), i, |ui| {
                    egui::Frame::new()
                        .fill(tokens::surface_muted())
                        .stroke(Stroke::new(1.0_f32, tokens::line()))
                        .corner_radius(theme::radius::MD)
                        .inner_margin(egui::Margin::symmetric(10, 3))
                        .show(ui, |ui| ui.label(label));
                });
                let resp = chip.response.on_hover_text(t.drag_part);
                // Drop before or after this part, depending on the pointer's side; a line marks the spot.
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos())
                    && resp.dnd_hover_payload::<usize>().is_some()
                {
                    let after = pos.x > resp.rect.center().x;
                    let x = if after { resp.rect.right() + 2.0 } else { resp.rect.left() - 2.0 };
                    ui.painter().vline(x, resp.rect.y_range(), Stroke::new(2.0_f32, tokens::accent()));
                    if let Some(from) = resp.dnd_release_payload::<usize>() {
                        moved = Some((*from, if after { i + 1 } else { i }));
                    }
                }
                let x = egui::Button::new(theme::icon_text(theme::icon::X, 12.0, tokens::muted()))
                    .frame(false)
                    .min_size(Vec2::new(20.0, 24.0));
                if ui.add(x).on_hover_text((t.remove_part)(&b.label())).clicked() {
                    removed = Some(i);
                }
            });
        }
        if let Some(b) = add_menu(ui, blocks) {
            blocks.push(b);
            changed = true;
        }
    });
    if let Some((from, to)) = moved
        && from != to
        && from + 1 != to
    {
        move_block(blocks, from, to);
        changed = true;
    }
    if let Some(i) = removed {
        blocks.remove(i);
        changed = true;
    }
    if blocks.is_empty() {
        ui.label(RichText::new(t.no_parts).size(text::SMALL).color(tokens::status_warn()));
    }
    changed
}

/// The add button: missing parts or custom text.
fn add_menu(ui: &mut egui::Ui, blocks: &[Block]) -> Option<Block> {
    let t = t();
    let mut chosen = None;
    let button =
        theme::chip_button(RichText::new(t.add_part).size(text::SMALL).color(tokens::accent()), theme::COMPACT);
    let text_id = egui::Id::unique("name-own-text");
    egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        theme::list_style(ui);
        ui.set_min_width(220.0);
        for key in FIELDS {
            let present =
                blocks.iter().any(|b| matches!(b, Block::Field(f) if f == key || (key == "subject" && f == "title")));
            if !present && ui.button(field_label(key).unwrap_or(key)).clicked() {
                chosen = Some(Block::Field(key.to_string()));
                ui.close();
            }
        }
        ui.separator();
        ui.label(theme::field_label(t.own_text));
        let mut own: String = ui.data(|d| d.get_temp(text_id)).unwrap_or_default();
        let resp =
            theme::field(ui, TextEdit::singleline(&mut own).hint_text(t.own_text_placeholder).desired_width(200.0));
        let clean = own.trim().replace(['{', '}'], "");
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !clean.is_empty() && (ui.button(t.add).clicked() || enter) {
            chosen = Some(Block::Text(clean));
            own.clear();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(text_id, own));
    });
    chosen
}

/// Separator choice; locked for a custom template.
pub fn separator(ui: &mut egui::Ui, template: &mut String) {
    let parsed = parse(template);
    let current = parsed.as_ref().map(|(_, s)| *s);
    let shown = current.map_or("–", separator_label);
    ui.add_enabled_ui(parsed.is_some(), |ui| {
        let mut new_sep = current.unwrap_or_default();
        theme::combo("name-sep").selected_text(shown).width(340.0).show_ui(ui, |ui| {
            for s in SEPARATORS {
                ui.selectable_value(&mut new_sep, s, separator_label(s));
            }
        });
        if let Some((blocks, sep)) = &parsed
            && new_sep != *sep
        {
            *template = compose(blocks, new_sep);
        }
    })
    .response
    .on_disabled_hover_text(t().separator_in_template);
}

/// The template as text, next to it the quick-start templates.
pub fn text_row(ui: &mut egui::Ui, template: &mut String) {
    let t = t();
    ui.horizontal(|ui| {
        theme::field(
            ui,
            TextEdit::singleline(template)
                .font(egui::TextStyle::Monospace)
                .desired_width(470.0)
                .hint_text("{date}_{doctype}_{sender}"),
        )
        .on_hover_text(t.template_hint);
        theme::combo("name-preset").selected_text(t.templates).width(110.0).show_ui(ui, |ui| {
            ui.set_min_width(320.0);
            for (preset, label) in PRESETS.into_iter().zip(t.presets) {
                ui.selectable_value(template, preset.to_string(), label);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> Block {
        Block::Field(s.into())
    }

    #[test]
    fn templates_become_blocks_and_back() {
        let (b, sep) = parse("{date}_{doctype}_{sender}").unwrap();
        assert_eq!(b, [f("date"), f("doctype"), f("sender")]);
        assert_eq!(sep, "_");
        assert_eq!(compose(&b, "-"), "{date}-{doctype}-{sender}");
        let (b, sep) = parse("{sender} Scan {date}").unwrap();
        assert_eq!((b.len(), sep), (3, " "));
        assert_eq!(b[1], Block::Text("Scan".into()));
        assert_eq!(parse("{sender}_{title}_{year}-{month}"), None, "two parts in one piece");
        assert_eq!(parse("{date}_{unbekannt}"), None);
        assert_eq!(parse("nur Text"), None, "no part");
        assert_eq!(parse("").map(|(b, _)| b.len()), Some(0));
        for t in PRESETS {
            assert_eq!(parse(t).map(|(b, s)| compose(&b, s)).as_deref(), Some(t), "{t}");
        }
    }

    #[test]
    fn blocks_move_before_the_target() {
        let mut b = vec![f("date"), f("doctype"), f("sender")];
        move_block(&mut b, 2, 0);
        assert_eq!(b, [f("sender"), f("date"), f("doctype")]);
        move_block(&mut b, 0, 3);
        assert_eq!(b, [f("date"), f("doctype"), f("sender")], "to the end");
        move_block(&mut b, 0, 2);
        assert_eq!(b, [f("doctype"), f("date"), f("sender")]);
    }
}
