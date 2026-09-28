mod model;

use eframe::{emath, egui::{
    self, Align, Color32, ComboBox, CornerRadius, Frame, Id, Layout, Margin, RichText, ScrollArea,
    Stroke, TextEdit, Vec2,
}};
use jiff::{Zoned, civil::Date};
use model::{Board, Card, SortMode, ThemeChoice};
use std::collections::HashMap;

/// How long a checked-off card lingers (faded out) before moving to the archive.
const ARCHIVE_DELAY: f64 = 2.5;
const COLUMN_WIDTH: f32 = 290.0;

const PALETTE: [(&str, [u8; 3]); 8] = [
    ("Red", [229, 72, 77]),
    ("Orange", [242, 140, 40]),
    ("Yellow", [230, 190, 40]),
    ("Green", [70, 167, 88]),
    ("Teal", [18, 165, 148]),
    ("Blue", [62, 99, 221]),
    ("Purple", [142, 78, 198]),
    ("Pink", [214, 64, 159]),
];

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Payload carried while dragging a card.
struct DragCard(u64);

/// UI changes are collected during rendering and applied afterwards to keep the borrow checker happy.
enum Action {
    Toggle(u64),
    Edit(u64),
    NewCard(usize),
    Move { card: u64, to_col: usize, before: Option<u64> },
    DeleteCard(u64),
    RenameColumn(usize),
    DeleteColumn(usize),
    ShiftColumn(usize, isize),
}

struct Editor {
    card: Card,
    tags: String,
    has_due: bool,
    due: Date,
    is_new: bool,
    col: usize,
}

struct KanbanApp {
    board: Board,
    /// card id -> time (egui seconds) it was checked off.
    pending: HashMap<u64, f64>,
    editor: Option<Editor>,
    renaming: Option<(usize, String)>,
    focus_rename: bool,
    show_archive: bool,
    filter: String,
    dirty: bool,
}

impl KanbanApp {
    fn new(cc: &eframe::CreationContext) -> Self {
        let board = Board::load();
        apply_theme(&cc.egui_ctx, board.theme);
        cc.egui_ctx.all_styles_mut(|s| s.spacing.item_spacing = Vec2::new(8.0, 6.0));
        Self {
            board,
            pending: HashMap::new(),
            editor: None,
            renaming: None,
            focus_rename: false,
            show_archive: false,
            filter: String::new(),
            dirty: false,
        }
    }

    fn matches_filter(&self, card: &Card) -> bool {
        let f = self.filter.trim().to_lowercase();
        f.is_empty()
            || card.title.to_lowercase().contains(&f)
            || card.description.to_lowercase().contains(&f)
            || card.tags.iter().any(|t| t.to_lowercase().contains(&f))
    }

    fn apply(&mut self, action: Action) {
        let b = &mut self.board;
        match action {
            Action::Toggle(id) => {
                if self.pending.remove(&id).is_none() {
                    self.pending.insert(id, f64::NAN); // timestamp filled in by the caller
                }
                return;
            }
            Action::Edit(id) => {
                if let Some((ci, i)) = b.find(id) {
                    let card = b.columns[ci].cards[i].clone();
                    self.editor = Some(Editor {
                        tags: card.tags.join(", "),
                        has_due: card.due.is_some(),
                        due: card.due.unwrap_or_else(today),
                        card,
                        is_new: false,
                        col: ci,
                    });
                }
                return;
            }
            Action::NewCard(ci) => {
                let card = Card { id: b.new_id(), ..Default::default() };
                self.editor = Some(Editor {
                    card,
                    tags: String::new(),
                    has_due: false,
                    due: today(),
                    is_new: true,
                    col: ci,
                });
            }
            Action::Move { card, to_col, before } => {
                if let Some((_, c)) = b.take(card) {
                    let cards = &mut b.columns[to_col].cards;
                    let at = before.and_then(|t| cards.iter().position(|k| k.id == t)).unwrap_or(cards.len());
                    cards.insert(at, c);
                }
            }
            Action::DeleteCard(id) => {
                b.take(id);
            }
            Action::RenameColumn(ci) => {
                self.renaming = Some((ci, b.columns[ci].title.clone()));
                self.focus_rename = true;
                return;
            }
            Action::DeleteColumn(ci) => {
                let col = b.columns.remove(ci);
                // Don't silently lose cards: archive them.
                for card in col.cards {
                    b.archive.insert(0, model::Archived {
                        card,
                        column: col.title.clone(),
                        archived_at: jiff::Timestamp::now(),
                    });
                }
            }
            Action::ShiftColumn(ci, d) => {
                let j = ci as isize + d;
                if j >= 0 && (j as usize) < b.columns.len() {
                    b.columns.swap(ci, j as usize);
                }
            }
        }
        self.dirty = true;
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Kanban");
            ui.separator();
            if ui.button("➕ Column").clicked() {
                self.board.add_column("New column");
                self.renaming = Some((self.board.columns.len() - 1, "New column".into()));
                self.focus_rename = true;
                self.dirty = true;
            }
            ui.add(TextEdit::singleline(&mut self.filter).hint_text("🔍 Filter by text or tag").desired_width(200.0));
            if !self.filter.is_empty() && ui.small_button("✖").clicked() {
                self.filter.clear();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button("⚙ Settings", |ui| {
                    ui.label(RichText::new("Theme").strong());
                    for t in ThemeChoice::ALL {
                        if ui.radio_value(&mut self.board.theme, t, t.label()).clicked() {
                            apply_theme(ui.ctx(), t);
                            self.dirty = true;
                        }
                    }
                    ui.separator();
                    ui.weak(format!("Data file:\n{}", Board::path().display()));
                    ui.separator();
                    ui.label(RichText::new("About").strong());
                    ui.label(format!("Kanban v{}", env!("CARGO_PKG_VERSION")));
                    ui.label("Made by Reed Graf");
                    ui.hyperlink_to("GitHub: reedgraf", "https://github.com/reedgraf");
                });
                let label = format!("🗄 Archive ({})", self.board.archive.len());
                ui.toggle_value(&mut self.show_archive, label);
            });
        });
    }

    fn archive_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Archive");
            if !self.board.archive.is_empty() && ui.small_button("Clear all").clicked() {
                self.board.archive.clear();
                self.dirty = true;
            }
        });
        ui.separator();
        if self.board.archive.is_empty() {
            ui.weak("Checked-off cards end up here.");
        }
        let mut restore = None;
        let mut delete = None;
        ScrollArea::vertical().id_salt("archive").show(ui, |ui| {
            let tz = jiff::tz::TimeZone::system();
            for (i, a) in self.board.archive.iter().enumerate() {
                Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&a.card.title).strong());
                    let when = a.archived_at.to_zoned(tz.clone()).strftime("%Y-%m-%d %H:%M");
                    ui.weak(format!("from {} · {when}", a.column));
                    ui.horizontal(|ui| {
                        if ui.small_button("↩ Restore").clicked() {
                            restore = Some(i);
                        }
                        if ui.small_button("🗑 Delete").clicked() {
                            delete = Some(i);
                        }
                    });
                });
            }
        });
        if let Some(i) = restore {
            self.board.restore(i);
            self.dirty = true;
        }
        if let Some(i) = delete {
            self.board.archive.remove(i);
            self.dirty = true;
        }
    }

    fn board_ui(&mut self, ui: &mut egui::Ui, now: f64, actions: &mut Vec<Action>) {
        let today = today();
        ScrollArea::horizontal().id_salt("board").show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let ncols = self.board.columns.len();
                for ci in 0..ncols {
                    self.column_ui(ui, ci, ncols, now, today, actions);
                }
            });
        });
    }

    fn column_ui(&mut self, ui: &mut egui::Ui, ci: usize, ncols: usize, now: f64, today: Date, actions: &mut Vec<Action>) {
        let height = ui.available_height();
        let stroke = match self.board.columns[ci].color {
            Some(c) => Stroke::new(2.5, rgb(c)),
            None => ui.visuals().widgets.noninteractive.bg_stroke,
        };
        let frame = Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .stroke(stroke)
            .corner_radius(8)
            .inner_margin(10);

        let mut prepared = frame.begin(ui);
        prepared.content_ui.vertical(|ui| {
            ui.set_width(COLUMN_WIDTH);
            ui.set_min_height(height - 24.0);

            // Header
            ui.horizontal(|ui| {
                let renaming_this = matches!(&self.renaming, Some((i, _)) if *i == ci);
                if renaming_this {
                    let (_, text) = self.renaming.as_mut().unwrap();
                    let r = ui.add(TextEdit::singleline(text).id_salt(("rename", ci)).desired_width(150.0));
                    // Grab focus only when renaming starts; doing it every frame would never let go.
                    if std::mem::take(&mut self.focus_rename) {
                        r.request_focus();
                    }
                    // Enter or clicking elsewhere saves, Esc cancels.
                    if r.lost_focus() {
                        let (_, text) = self.renaming.take().unwrap();
                        let text = text.trim();
                        if !text.is_empty() && !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.board.columns[ci].title = text.to_string();
                            self.dirty = true;
                        }
                    }
                } else {
                    let col = &self.board.columns[ci];
                    let r = ui.add(
                        egui::Label::new(RichText::new(&col.title).strong().size(16.0)).sense(egui::Sense::click()),
                    );
                    ui.weak(col.cards.len().to_string());
                    if r.double_clicked() {
                        actions.push(Action::RenameColumn(ci));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("☰", |ui| {
                        if ui.button("Rename").clicked() {
                            actions.push(Action::RenameColumn(ci));
                        }
                        if ci > 0 && ui.button("Move left").clicked() {
                            actions.push(Action::ShiftColumn(ci, -1));
                        }
                        if ci + 1 < ncols && ui.button("Move right").clicked() {
                            actions.push(Action::ShiftColumn(ci, 1));
                        }
                        ui.separator();
                        ui.label("Border color");
                        ui.horizontal(|ui| {
                            let col = &mut self.board.columns[ci];
                            if ui.selectable_label(col.color.is_none(), "None").clicked() {
                                col.color = None;
                                self.dirty = true;
                            }
                            for (name, c) in PALETTE {
                                let selected = col.color == Some(c);
                                let text = RichText::new(if selected { "⏺" } else { "⬤" }).color(rgb(c)).size(18.0);
                                if ui.add(egui::Button::new(text).frame(selected)).on_hover_text(name).clicked() {
                                    col.color = Some(c);
                                    self.dirty = true;
                                }
                            }
                        });
                        ui.separator();
                        if ui.button("Delete column (archives its cards)").clicked() {
                            actions.push(Action::DeleteColumn(ci));
                        }
                    });
                    if ui.button("➕").on_hover_text("Add card").clicked() {
                        actions.push(Action::NewCard(ci));
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.weak("Sort:");
                let col = &mut self.board.columns[ci];
                let before = col.sort;
                ComboBox::from_id_salt(("sort", col.id)).selected_text(col.sort.label()).show_ui(ui, |ui| {
                    for m in SortMode::ALL {
                        ui.selectable_value(&mut col.sort, m, m.label());
                    }
                });
                if col.sort != before {
                    self.dirty = true;
                }
            });
            ui.separator();

            // Cards
            let manual = self.board.columns[ci].sort == SortMode::Manual;
            ScrollArea::vertical().id_salt(("cards", ci)).auto_shrink([false, false]).show(ui, |ui| {
                let col = &self.board.columns[ci];
                for i in col.display_order() {
                    let card = &col.cards[i];
                    if !self.matches_filter(card) {
                        continue;
                    }
                    let progress = self.pending.get(&card.id).map(|t| ((now - t) / ARCHIVE_DELAY).clamp(0.0, 1.0) as f32);
                    let resp = card_ui(ui, card, progress, today, actions);
                    // Dropping onto a card inserts before it (only meaningful when manually ordered).
                    if manual && let Some(p) = resp.dnd_release_payload::<DragCard>() && p.0 != card.id {
                        actions.push(Action::Move { card: p.0, to_col: ci, before: Some(card.id) });
                    }
                    if manual && resp.dnd_hover_payload::<DragCard>().is_some() {
                        let r = resp.rect;
                        ui.painter().hline(r.x_range(), r.top() - 3.0, Stroke::new(2.0, ui.visuals().selection.bg_fill));
                    }
                }
            });
        });
        let response = prepared.allocate_space(ui);
        // Highlight the column a card is being dragged over.
        if response.dnd_hover_payload::<DragCard>().is_some() {
            prepared.frame.fill = ui.visuals().widgets.hovered.weak_bg_fill;
        }
        prepared.paint(ui);

        if let Some(p) = response.dnd_release_payload::<DragCard>() {
            // Released on the column itself (not on a specific card): append, unless already handled.
            let already = actions.iter().any(|a| matches!(a, Action::Move { card, .. } if *card == p.0));
            let same_col = self.board.find(p.0).map(|(c, _)| c) == Some(ci);
            if !already && !(same_col && !self.board.columns[ci].cards.is_empty() && self.board.columns[ci].sort != SortMode::Manual) {
                actions.push(Action::Move { card: p.0, to_col: ci, before: None });
            }
        }
    }

    fn editor_window(&mut self, ctx: &egui::Context) {
        let Some(ed) = &mut self.editor else { return };
        let mut open = true;
        let mut result: Option<bool> = None; // Some(true)=save, Some(false)=delete
        let title = if ed.is_new { "New card" } else { "Edit card" };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("editor").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Title");
                    let r = ui.add(TextEdit::singleline(&mut ed.card.title).desired_width(280.0));
                    if ed.is_new && ed.card.title.is_empty() && !r.has_focus() {
                        r.request_focus();
                    }
                    ui.end_row();

                    ui.label("Description");
                    ui.add(TextEdit::multiline(&mut ed.card.description).desired_width(280.0).desired_rows(5));
                    ui.end_row();

                    ui.label("Due date");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut ed.has_due, "");
                        ui.add_enabled_ui(ed.has_due, |ui| {
                            ui.add(egui_extras::DatePickerButton::new(&mut ed.due).id_salt("due"));
                        });
                    });
                    ui.end_row();

                    ui.label("Color");
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut ed.card.color, None, "None");
                        for (name, c) in PALETTE {
                            let selected = ed.card.color == Some(c);
                            let text = RichText::new(if selected { "⏺" } else { "⬤" }).color(rgb(c)).size(18.0);
                            if ui.add(egui::Button::new(text).frame(selected)).on_hover_text(name).clicked() {
                                ed.card.color = Some(c);
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Tags");
                    ui.add(TextEdit::singleline(&mut ed.tags).hint_text("comma, separated").desired_width(280.0));
                    ui.end_row();
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let can_save = !ed.card.title.trim().is_empty();
                    if ui.add_enabled(can_save, egui::Button::new("💾 Save")).clicked()
                        || (can_save && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl))
                    {
                        result = Some(true);
                    }
                    if !ed.is_new && ui.button("🗑 Delete").clicked() {
                        result = Some(false);
                    }
                    ui.weak("Ctrl+Enter to save");
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }

        match result {
            Some(true) => {
                let mut ed = self.editor.take().unwrap();
                ed.card.title = ed.card.title.trim().to_string();
                ed.card.due = ed.has_due.then_some(ed.due);
                ed.card.tags = ed.tags.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                ed.card.tags.dedup();
                if let Some((ci, i)) = self.board.find(ed.card.id) {
                    self.board.columns[ci].cards[i] = ed.card;
                } else if let Some(col) = self.board.columns.get_mut(ed.col) {
                    col.cards.push(ed.card);
                }
                self.dirty = true;
            }
            Some(false) => {
                let id = ed.card.id;
                self.editor = None;
                self.apply(Action::DeleteCard(id));
            }
            None if !open => self.editor = None,
            None => {}
        }
    }
}

fn apply_theme(ctx: &egui::Context, t: ThemeChoice) {
    ctx.set_theme(match t {
        ThemeChoice::System => egui::ThemePreference::System,
        ThemeChoice::Dark => egui::ThemePreference::Dark,
        ThemeChoice::Light => egui::ThemePreference::Light,
    });
}

fn today() -> Date {
    Zoned::now().date()
}

/// Draws one card. Returns the response covering the whole card (used for drag & drop).
fn card_ui(ui: &mut egui::Ui, card: &Card, progress: Option<f32>, today: Date, actions: &mut Vec<Action>) -> egui::Response {
    let checked = progress.is_some();
    let fade = 1.0 - progress.unwrap_or(0.0) * 0.7;
    let v = ui.visuals();
    let frame = Frame::new()
        .fill(v.extreme_bg_color)
        .stroke(v.widgets.noninteractive.bg_stroke)
        .corner_radius(6)
        .inner_margin(Margin { left: 12, right: 8, top: 8, bottom: 8 });

    // Our own drag source (instead of `ui.dnd_drag_source`): its drag sense is registered *below*
    // the card's contents, so the checkbox and edit button stay clickable and don't start a drag.
    let id = Id::new(("card", card.id));
    let ctx = ui.ctx().clone();
    let dragging = ctx.is_being_dragged(id);
    let mut over_control = false;
    let builder = if dragging {
        egui::DragAndDrop::set_payload(&ctx, DragCard(card.id));
        // Paint the card on a top layer so it can follow the pointer.
        egui::UiBuilder::new().layer_id(egui::LayerId::new(egui::Order::Tooltip, id))
    } else {
        egui::UiBuilder::new().id(id).sense(egui::Sense::click_and_drag())
    };

    let inner = ui.scope_builder(builder, |ui| {
        ui.set_opacity(fade);
        let r = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let mut c = checked;
                let r = ui.checkbox(&mut c, "").on_hover_text("Mark done (moves to archive)");
                over_control |= claim_drag(ui, &r);
                if r.changed() {
                    actions.push(Action::Toggle(card.id));
                }
                let mut title = RichText::new(&card.title).strong();
                if checked {
                    title = title.strikethrough();
                }
                let w = ui.available_width() - 28.0;
                ui.scope(|ui| {
                    ui.set_max_width(w);
                    ui.add(egui::Label::new(title).wrap());
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let r = ui.small_button("✏").on_hover_text("Edit");
                    over_control |= claim_drag(ui, &r);
                    if r.clicked() {
                        actions.push(Action::Edit(card.id));
                    }
                });
            });
            if !card.description.is_empty() {
                ui.add(egui::Label::new(RichText::new(&card.description).weak()).wrap());
            }
            if card.due.is_some() || !card.tags.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if let Some(due) = card.due {
                        let days = (due - today).get_days();
                        let (color, text) = match days {
                            d if d < 0 => (Color32::from_rgb(229, 72, 77), format!("⏰ {due} (overdue)")),
                            0 => (Color32::from_rgb(242, 140, 40), format!("⏰ {due} (today)")),
                            1 => (Color32::from_rgb(242, 140, 40), format!("⏰ {due} (tomorrow)")),
                            _ => (ui.visuals().weak_text_color(), format!("📅 {due}")),
                        };
                        ui.label(RichText::new(text).small().color(color));
                    }
                    for tag in &card.tags {
                        tag_chip(ui, tag);
                    }
                });
            }
        });
        // Color label: a stripe down the left edge of the card.
        if let Some(c) = card.color {
            let rect = r.response.rect;
            let stripe = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 5.0, rect.max.y));
            ui.painter().rect_filled(stripe, CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, rgb(c));
        }
    });

    if dragging {
        if let Some(pos) = ctx.pointer_interact_pos() {
            let delta = pos - inner.response.rect.center();
            ctx.transform_layer_shapes(egui::LayerId::new(egui::Order::Tooltip, id), emath::TSTransform::from_translation(delta));
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if inner.response.hovered() && !over_control {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }
    inner.response.context_menu(|ui| {
        if ui.button("Edit").clicked() {
            actions.push(Action::Edit(card.id));
        }
        if ui.button(if checked { "Undo check" } else { "Mark done" }).clicked() {
            actions.push(Action::Toggle(card.id));
        }
        if ui.button("Delete").clicked() {
            actions.push(Action::DeleteCard(card.id));
        }
    });
    if inner.response.double_clicked() {
        actions.push(Action::Edit(card.id));
    }
    inner.response
}

/// Make a control inside a card also sense drags, so dragging on it doesn't move the card.
/// Returns whether the pointer is over it.
fn claim_drag(ui: &egui::Ui, r: &egui::Response) -> bool {
    ui.interact(r.rect, r.id, egui::Sense::click_and_drag());
    r.hovered()
}

fn tag_chip(ui: &mut egui::Ui, tag: &str) {
    // Stable per-tag hue so the same tag always looks the same.
    let hue = tag.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 360;
    let base: Color32 = egui::ecolor::Hsva::new(hue as f32 / 360.0, 0.55, 0.75, 1.0).into();
    Frame::new()
        .fill(base.gamma_multiply(0.25))
        .stroke(Stroke::new(1.0, base.gamma_multiply(0.7)))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(RichText::new(tag).small()));
}

impl eframe::App for KanbanApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);

        // Move checked-off cards to the archive once their delay has passed.
        let due: Vec<u64> = self.pending.iter().filter(|(_, t)| now - **t >= ARCHIVE_DELAY).map(|(id, _)| *id).collect();
        for id in due {
            self.pending.remove(&id);
            self.board.archive_card(id);
            self.dirty = true;
        }
        if !self.pending.is_empty() {
            ctx.request_repaint(); // keep the fade animation running
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(2.0);
        });
        if self.show_archive {
            egui::Panel::right("archive").resizable(true).default_size(280.0).show(ui, |ui| self.archive_panel(ui));
        }
        let mut actions = Vec::new();
        egui::CentralPanel::default_margins().show(ui, |ui| self.board_ui(ui, now, &mut actions));
        self.editor_window(&ctx);

        for a in actions {
            self.apply(a);
        }
        for t in self.pending.values_mut() {
            if t.is_nan() {
                *t = now;
            }
        }

        if self.dirty {
            self.board.save();
            self.dirty = false;
        }
    }

    fn on_exit(&mut self) {
        // Anything still waiting on its delay is archived right away.
        for id in self.pending.keys().copied().collect::<Vec<_>>() {
            self.board.archive_card(id);
        }
        self.board.save();
    }
}

const APP_ID: &str = "io.github.reedgraf.kanban";
const ICON_PNG: &[u8] = include_bytes!("../assets/io.github.reedgraf.kanban.png");
const ICON_SVG: &str = include_str!("../assets/io.github.reedgraf.kanban.svg");
const DESKTOP_ENTRY: &str = include_str!("../assets/io.github.reedgraf.kanban.desktop");
/// Marks launcher files the app wrote itself, so it never overwrites one a package or `make install` put there.
const GENERATED_MARK: &str = "X-Kanban-Generated=true";

/// Wayland has no way for an app to set its window icon: the compositor looks up the `.desktop`
/// file named after the window's app id and uses its icon. When the binary is run without being
/// installed, write a per-user launcher and icon to ~/.local/share so the icon shows up.
fn ensure_desktop_entry() {
    let Some(data_home) = dirs::data_dir() else { return };
    let rel = format!("applications/{APP_ID}.desktop");
    let system_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    if system_dirs.split(':').filter(|d| !d.is_empty()).any(|d| std::path::Path::new(d).join(&rel).exists()) {
        return; // installed from a package
    }
    let desktop_path = data_home.join(&rel);
    let existing = std::fs::read_to_string(&desktop_path).ok();
    if existing.as_ref().is_some_and(|e| !e.contains(GENERATED_MARK)) {
        return; // installed with `make install PREFIX=~/.local`, or edited by the user
    }
    let Ok(exe) = std::env::current_exe() else { return };
    // Desktop-entry quoting: `"`, `` ` ``, `$` and `\` get a backslash (doubled, since the value is
    // itself unescaped once), and `%` becomes `%%`.
    let mut exec = String::from("\"");
    for c in exe.display().to_string().chars() {
        match c {
            '"' | '`' | '$' | '\\' => exec.extend(['\\', '\\', c]),
            '%' => exec.push_str("%%"),
            c => exec.push(c),
        }
    }
    exec.push('"');
    let icon_path = data_home.join(format!("icons/hicolor/scalable/apps/{APP_ID}.svg"));
    // Use the icon's absolute path rather than its theme name: long-running shells (e.g. Plasma's
    // taskbar) cache the icon theme at startup and won't see an icon folder created after that.
    let entry = DESKTOP_ENTRY
        .replace("Exec=kanban", &format!("Exec={exec}"))
        .replace(&format!("Icon={APP_ID}"), &format!("Icon={}", icon_path.display()))
        + GENERATED_MARK
        + "\n";
    let write = |path: &std::path::Path, contents: &str| -> std::io::Result<()> {
        if std::fs::read_to_string(path).ok().as_deref() != Some(contents) {
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, contents)?;
        }
        Ok(())
    };
    if let Err(e) = write(&icon_path, ICON_SVG).and_then(|_| write(&desktop_path, &entry)) {
        eprintln!("kanban: could not install desktop entry: {e}");
    }
}

fn main() -> eframe::Result {
    ensure_desktop_entry();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Kanban")
        .with_app_id(APP_ID)
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([500.0, 350.0]);
    // Used on X11 and other platforms; Wayland relies on the desktop entry above.
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("Kanban", options, Box::new(|cc| Ok(Box::new(KanbanApp::new(cc)))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, Pos2, RawInput, pos2, vec2};

    /// Headless harness: renders one card and feeds it pointer events.
    struct Harness {
        ctx: egui::Context,
        card: Card,
        actions: Vec<Action>,
    }

    impl Harness {
        fn new() -> Self {
            let card = Card { id: 7, title: "Card".into(), description: "Some description".into(), ..Default::default() };
            let mut h = Harness { ctx: egui::Context::default(), card, actions: vec![] };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) -> egui::Rect {
            let mut rect = egui::Rect::NOTHING;
            let (card, actions) = (&self.card, &mut self.actions);
            let mut out = self.ctx.run_ui(RawInput { events, ..Default::default() }, |ui| {
                egui::CentralPanel::default_margins().show(ui, |ui| {
                    ui.set_width(300.0);
                    rect = card_ui(ui, card, None, today(), actions).rect;
                });
            });
            out.textures_delta.clear();
            rect
        }

        fn press(&mut self, p: Pos2, pressed: bool) {
            self.frame(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() }]);
        }

        /// Press at `p`, drag 40px down; returns whether the card was being dragged.
        fn drag(&mut self, p: Pos2) -> bool {
            self.frame(vec![Event::PointerMoved(p)]);
            self.press(p, true);
            for i in 1..=4 {
                self.frame(vec![Event::PointerMoved(p + vec2(0.0, 10.0 * i as f32))]);
            }
            let dragging = self.ctx.is_being_dragged(Id::new(("card", 7u64)));
            self.press(p + vec2(0.0, 40.0), false);
            dragging
        }

        fn edit_button_pos(&mut self) -> Pos2 {
            let r = self.frame(vec![]);
            pos2(r.right() - 18.0, r.top() + 16.0)
        }
    }

    #[test]
    fn body_drags_card() {
        let mut h = Harness::new();
        let r = h.frame(vec![]);
        assert!(h.drag(pos2(r.center().x, r.bottom() - 6.0)));
    }

    #[test]
    fn edit_button_does_not_drag_card() {
        let mut h = Harness::new();
        let p = h.edit_button_pos();
        assert!(!h.drag(p));
    }

    #[test]
    fn edit_button_clicks() {
        let mut h = Harness::new();
        let p = h.edit_button_pos();
        h.frame(vec![Event::PointerMoved(p)]);
        h.press(p, true);
        h.press(p, false);
        assert!(h.actions.iter().any(|a| matches!(a, Action::Edit(_))));
    }

    fn app() -> KanbanApp {
        KanbanApp {
            board: Board::default(),
            pending: HashMap::new(),
            editor: None,
            renaming: None,
            focus_rename: false,
            show_archive: false,
            filter: String::new(),
            dirty: false,
        }
    }

    fn app_frame(ctx: &egui::Context, app: &mut KanbanApp, events: Vec<Event>) {
        let mut actions = Vec::new();
        let mut out = ctx.run_ui(RawInput { events, ..Default::default() }, |ui| {
            egui::CentralPanel::default_margins().show(ui, |ui| app.board_ui(ui, 0.0, &mut actions));
        });
        out.textures_delta.clear();
        for a in actions {
            app.apply(a);
        }
    }

    fn key(k: egui::Key) -> Event {
        Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }
    }

    #[test]
    fn rename_saves_on_enter_and_releases_focus() {
        let ctx = egui::Context::default();
        let mut a = app();
        a.apply(Action::RenameColumn(0));
        app_frame(&ctx, &mut a, vec![]);
        app_frame(&ctx, &mut a, vec![Event::Text("!".into())]);
        app_frame(&ctx, &mut a, vec![key(egui::Key::Enter)]);
        app_frame(&ctx, &mut a, vec![]);
        assert!(a.renaming.is_none(), "rename box should close");
        assert_eq!(a.board.columns[0].title, "To do!");
        assert!(ctx.memory(|m| m.focused()).is_none(), "focus should be released");
    }

    #[test]
    fn rename_escape_cancels() {
        let ctx = egui::Context::default();
        let mut a = app();
        a.apply(Action::RenameColumn(0));
        app_frame(&ctx, &mut a, vec![]);
        app_frame(&ctx, &mut a, vec![Event::Text("xyz".into())]);
        app_frame(&ctx, &mut a, vec![key(egui::Key::Escape)]);
        app_frame(&ctx, &mut a, vec![]);
        assert!(a.renaming.is_none());
        assert_eq!(a.board.columns[0].title, "To do");
    }
}
