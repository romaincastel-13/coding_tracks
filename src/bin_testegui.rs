use eframe::egui;
use egui::{Event::MouseWheel, Key::V};
mod common;
use crate::common::*;
use std::collections::{HashMap, HashSet, VecDeque};

// fn main_simple() -> eframe::Result {
//     // env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
//     let options = eframe::NativeOptions {
//         viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
//         ..Default::default()
//     };
//     eframe::run_native(
//         "My egui App",
//         options,
//         Box::new(|cc| {
//             // This gives us image support:
//             // egui_extras::install_image_loaders(&cc.egui_ctx);

//             Ok(Box::<MyApp>::default())
//         }),
//     )
// }

// struct MyApp {
//     name: String,
//     age: u32,
// }

// impl Default for MyApp {
//     fn default() -> Self {
//         Self {
//             name: "Arthur".to_owned(),
//             age: 42,
//         }
//     }
// }

// impl eframe::App for MyApp {
//     fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
//         egui::CentralPanel::default().show(ui, |ui| {
//             ui.heading("My egui Application");
//             ui.horizontal(|ui| {
//                 let name_label = ui.label("Your name: ");
//                 ui.text_edit_singleline(&mut self.name)
//                     .labelled_by(name_label.id);
//             });
//             ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
//             if ui.button("Increment").clicked() {
//                 self.age += 1;
//             }
//             ui.label(format!("Hello '{}', age {}", self.name, self.age));

//             // ui.image(egui::include_image!(
//             //     "../../../crates/egui/assets/ferris.png"
//             // ));
//         });
//     }
// }

pub struct StateViewer {
    pub states: Vec<State>,
    pub current_state: usize,

    pub cell_size: f32,

    pub show_cell_ids: bool,
    pub show_debug: bool,
}

impl StateViewer {
    pub fn new(states: Vec<State>) -> Self {
        Self {
            states,
            current_state: 0,
            cell_size: 40.0,
            show_cell_ids: false,
            show_debug: false,
        }
    }

    fn current(&self) -> Option<&State> {
        self.states.get(self.current_state)
    }

    fn previous(&mut self) {
        self.current_state = self.current_state.saturating_sub(1);
    }

    fn next(&mut self) {
        if self.current_state + 1 < self.states.len() {
            self.current_state += 1;
        }
    }

    fn first(&mut self) {
        self.current_state = 0;
    }

    fn last(&mut self) {
        if !self.states.is_empty() {
            self.current_state = self.states.len() - 1;
        }
    }
}
impl eframe::App for StateViewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ui);

        self.toolbar(ui);

        ui.separator();

        ui.columns(2, |columns| {
            columns[0].set_min_width(400.0);
            columns[1].set_min_width(180.0);

            self.board(&mut columns[0]);
            self.info_panel(&mut columns[1]);
        });
    }
}

impl StateViewer {
    fn handle_keyboard(&mut self, ui: &egui::Ui) {
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.previous();
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.next();
        }

        if ui.input(|i| i.key_pressed(egui::Key::Home)) {
            self.first();
        }

        if ui.input(|i| i.key_pressed(egui::Key::End)) {
            self.last();
        }

        if ui.input(|i| i.key_pressed(egui::Key::Plus)) {
            self.cell_size = (self.cell_size * 1.1).min(150.0);
        }

        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.cell_size = (self.cell_size / 1.1).max(10.0);
        }
    }
}

impl StateViewer {
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("⏮").clicked() {
                self.first();
            }

            if ui.button("◀").clicked() {
                self.previous();
            }

            if ui.button("▶").clicked() {
                self.next();
            }

            if ui.button("⏭").clicked() {
                self.last();
            }

            ui.separator();

            if self.states.is_empty() {
                ui.label("No states");
                return;
            }

            ui.label(format!(
                "State {} / {}",
                self.current_state + 1,
                self.states.len()
            ));

            let max = self.states.len() - 1;

            let mut value = self.current_state;

            if ui
                .add(egui::Slider::new(&mut value, 0..=max).show_value(false))
                .changed()
            {
                self.current_state = value;
            }

            ui.separator();

            ui.checkbox(&mut self.show_cell_ids, "IDs");
            ui.checkbox(&mut self.show_debug, "Debug");

            ui.separator();

            if ui.button("Zoom +").clicked() {
                self.cell_size = (self.cell_size * 1.1).min(150.0);
            }

            if ui.button("Zoom -").clicked() {
                self.cell_size = (self.cell_size / 1.1).max(10.0);
            }
        });
    }
}

impl StateViewer {
    fn board(&self, ui: &mut egui::Ui) {
        let Some(state) = self.current() else {
            ui.label("No state");
            return;
        };

        let width = state.width as f32 * self.cell_size;
        let height = state.height as f32 * self.cell_size;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (board_rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());

                let painter = ui.painter_at(board_rect);

                // ---------------------------------------------------------
                // 1. Draw cells
                // ---------------------------------------------------------
                for y in 0..state.height {
                    for x in 0..state.width {
                        let cell_id = y * state.width + x;

                        let Some(cell) = state.cells.get(cell_id) else {
                            continue;
                        };

                        let min = egui::pos2(
                            board_rect.left() + x as f32 * self.cell_size,
                            board_rect.top() + y as f32 * self.cell_size,
                        );

                        let rect = egui::Rect::from_min_size(
                            min,
                            egui::vec2(self.cell_size, self.cell_size),
                        );

                        self.draw_cell(&painter, rect, cell);
                    }
                }

                // ---------------------------------------------------------
                // 2. Draw towns AFTER cells
                //    Therefore towns are above terrain, tracks and ink.
                // ---------------------------------------------------------
                for town in &state.towns {
                    self.draw_town(&painter, board_rect, town);
                }
            });
    }

    fn draw_town(&self, painter: &egui::Painter, board_rect: egui::Rect, town: &Town) {
        let center = egui::pos2(
            board_rect.left() + (town.x as f32 + 0.5) * self.cell_size,
            board_rect.top() + (town.y as f32 + 0.5) * self.cell_size,
        );

        let radius = self.cell_size * 0.28;

        // Black outline.
        painter.circle_filled(center, radius + 2.0, egui::Color32::BLACK);

        // City circle.
        painter.circle_filled(center, radius, egui::Color32::WHITE);

        // House.
        let roof_height = radius * 0.65;
        let house_width = radius * 1.15;
        let house_height = radius * 0.9;

        let bottom = center.y + house_height * 0.45;
        let top = center.y - house_height * 0.25;

        let left = center.x - house_width / 2.0;
        let right = center.x + house_width / 2.0;

        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
            0.0,
            egui::Color32::from_gray(80),
        );

        // Roof.
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(left - radius * 0.1, top),
                egui::pos2(center.x, top - roof_height),
                egui::pos2(right + radius * 0.1, top),
            ],
            egui::Color32::from_gray(40),
            egui::Stroke::NONE,
        ));

        // ---------------------------------------------------------
        // City ID - always displayed on top of the city.
        // ---------------------------------------------------------
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            town.id.to_string(),
            egui::FontId::proportional((self.cell_size * 0.25).max(8.0)),
            egui::Color32::WHITE,
        );
    }
}

impl StateViewer {
    fn draw_cell(&self, painter: &egui::Painter, rect: egui::Rect, cell: &Cell) {
        // ============================================================
        // PRIORITY 1: INKED
        //
        // Inked completely hides whatever is underneath.
        // ============================================================

        if cell.inked {
            painter.rect_filled(rect, 0.0, egui::Color32::BLACK);

            self.draw_grid(painter, rect);

            if self.show_cell_ids {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    cell.cell_id.to_string(),
                    egui::FontId::proportional((self.cell_size * 0.25).max(8.0)),
                    egui::Color32::WHITE,
                );
            }

            return;
        }

        // ============================================================
        // PRIORITY 2: TRACK
        //
        // track_owner != -1 means there is a track.
        //
        // We draw the terrain first, then the track on top.
        // ============================================================

        if cell.track_owner != FREE {
            painter.rect_filled(rect, 0.0, terrain_color(cell.terrain));

            draw_track(
                painter,
                rect.center(),
                self.cell_size,
                track_color(cell.track_owner),
            );

            self.draw_grid(painter, rect);

            if self.show_cell_ids {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    cell.cell_id.to_string(),
                    egui::FontId::proportional((self.cell_size * 0.25).max(8.0)),
                    egui::Color32::WHITE,
                );
            }

            return;
        }

        // ============================================================
        // PRIORITY 3: TERRAIN
        // ============================================================

        painter.rect_filled(rect, 0.0, terrain_color(cell.terrain));

        self.draw_grid(painter, rect);

        if self.show_cell_ids {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                cell.cell_id.to_string(),
                egui::FontId::proportional((self.cell_size * 0.25).max(8.0)),
                egui::Color32::BLACK,
            );
        }

        if self.show_debug {
            painter.text(
                rect.left_top() + egui::vec2(2.0, 2.0),
                egui::Align2::LEFT_TOP,
                format!("r:{} i:{}", cell.region_id, cell.instability),
                egui::FontId::proportional((self.cell_size * 0.17).max(7.0)),
                egui::Color32::BLACK,
            );
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect) {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
            egui::StrokeKind::Inside,
        );
    }
}

fn terrain_color(terrain: i32) -> egui::Color32 {
    match terrain {
        PLAINS => egui::Color32::from_rgb(190, 170, 120),

        RIVER => egui::Color32::from_rgb(70, 140, 210),

        MOUNTAIN => egui::Color32::from_rgb(130, 130, 130),

        _ => egui::Color32::from_gray(100),
    }
}

fn track_color(owner: i32) -> egui::Color32 {
    match owner {
        0 => egui::Color32::from_rgb(220, 50, 50),

        1 => egui::Color32::from_rgb(50, 100, 230),

        NEUTRAL => egui::Color32::WHITE,

        _ => egui::Color32::LIGHT_GRAY,
    }
}

fn draw_track(painter: &egui::Painter, center: egui::Pos2, size: f32, color: egui::Color32) {
    let margin = size * 0.15;

    let left = center.x - size / 2.0 + margin;
    let right = center.x + size / 2.0 - margin;

    let top = center.y - size / 2.0 + margin;
    let bottom = center.y + size / 2.0 - margin;

    let rail_offset = size * 0.12;

    let rail = egui::Stroke::new((size * 0.07).max(2.0), color);

    // Rail 1
    painter.line_segment(
        [
            egui::pos2(left, top + rail_offset),
            egui::pos2(right, bottom + rail_offset),
        ],
        rail,
    );

    // Rail 2
    painter.line_segment(
        [
            egui::pos2(left, top - rail_offset),
            egui::pos2(right, bottom - rail_offset),
        ],
        rail,
    );

    // Sleepers
    let sleeper = egui::Stroke::new((size * 0.05).max(1.0), color);

    for i in 0..=5 {
        let t = i as f32 / 5.0;

        let x = left + (right - left) * t;
        let y = top + (bottom - top) * t;

        let dx = size * 0.10;
        let dy = size * 0.10;

        painter.line_segment(
            [egui::pos2(x - dx, y + dy), egui::pos2(x + dx, y - dy)],
            sleeper,
        );
    }
}

impl StateViewer {
    fn info_panel(&self, ui: &mut egui::Ui) {
        let Some(state) = self.current() else {
            ui.label("No state");
            return;
        };

        ui.heading("State");

        ui.label(format!("State: {}", self.current_state));

        ui.label(format!("My score: {}", state.my_score));

        ui.label(format!("Foe score: {}", state.foe_score));

        ui.separator();

        ui.label(format!("Board: {} × {}", state.width, state.height));

        ui.label(format!("Cells: {}", state.cells.len()));

        ui.label(format!("Towns: {}", state.towns.len()));

        ui.label(format!("Regions: {}", state.regions.len()));

        ui.separator();

        ui.label("Legend");

        legend(ui, terrain_color(PLAINS), "Plains");

        legend(ui, terrain_color(RIVER), "River");

        legend(ui, terrain_color(MOUNTAIN), "Mountain");

        ui.separator();

        legend(ui, egui::Color32::from_rgb(220, 50, 50), "Player 0");

        legend(ui, egui::Color32::from_rgb(50, 100, 230), "Player 1");

        legend(ui, egui::Color32::WHITE, "Neutral track");

        legend(ui, egui::Color32::BLACK, "Inked");
    }
}

fn legend(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());

        ui.painter().rect_filled(rect, 2.0, color);

        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
            egui::StrokeKind::Inside,
        );

        ui.label(text);
    });
}

fn main() -> eframe::Result<()> {
    let states = make_test_states();

    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Back Track King - State Viewer",
        options,
        Box::new(move |_cc| Ok(Box::new(StateViewer::new(states)))),
    )
}

fn make_test_states() -> Vec<State> {
    let width = 12;
    let height = 8;

    let mut states = Vec::new();

    for step in 0..20 {
        let mut cells = Vec::with_capacity(width * height);

        for y in 0..height {
            for x in 0..width {
                let cell_id = y * width + x;

                let terrain = if x == 5 {
                    RIVER
                } else if y == 2 || y == 6 {
                    MOUNTAIN
                } else {
                    PLAINS
                };

                // Some example tracks that grow as the state advances.
                let track_owner = if y == 4 && x <= step {
                    0
                } else if y == 5 && x <= step / 2 {
                    1
                } else {
                    FREE
                };

                // Example inked cell.
                let inked = step >= 12 && x == 8 && y == 3;

                cells.push(Cell {
                    region_id: 0,
                    cell_id,
                    terrain,
                    track_owner,
                    instability: 0,
                    inked,
                    active: Vec::new(),
                    town_id: None
                });
            }
        }

        let town = Town {
            id: 1,
            x: 2,
            y: 2,
            desired: Vec::new(),
        };

        states.push(State {
            my_id: 0,
            foe_id: 1,
            width,
            height,
            cells,
            towns: vec![town],
            regions: HashMap::new(),
            connections: HashMap::new(),
            my_score: step as i32,
            foe_score: (step / 2) as i32,
            top_cells: [0; 3]
        });
    }

    states
}
