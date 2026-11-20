#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod output;

use eframe::egui;
use engine::{spawn_mapper_thread, SharedState};
use mapper_core::actions::{ActionBinding, ActivatorType, GameAction, PhysicalButton};
use mapper_core::process_stick;
use std::sync::{Arc, Mutex};

const ALL_BUTTONS: [PhysicalButton; 8] = [
    PhysicalButton::South,
    PhysicalButton::East,
    PhysicalButton::West,
    PhysicalButton::North,
    PhysicalButton::LeftBumper,
    PhysicalButton::RightBumper,
    PhysicalButton::BackLeft,
    PhysicalButton::BackRight,
];

const ALL_ACTIONS: [GameAction; 5] = [
    GameAction::Jump,
    GameAction::Reload,
    GameAction::Interact,
    GameAction::QuickSave,
    GameAction::ToggleGyro,
];

fn button_label(btn: PhysicalButton) -> &'static str {
    match btn {
        PhysicalButton::South => "South (A / Cross)",
        PhysicalButton::East => "East (B / Circle)",
        PhysicalButton::West => "West (X / Square)",
        PhysicalButton::North => "North (Y / Triangle)",
        PhysicalButton::LeftBumper => "Left Bumper (LB / L1)",
        PhysicalButton::RightBumper => "Right Bumper (RB / R1)",
        PhysicalButton::LeftTriggerBtn => "Left Trigger (LT / L2)",
        PhysicalButton::RightTriggerBtn => "Right Trigger (RT / R2)",
        PhysicalButton::BackLeft => "Back Paddle Left (L4)",
        PhysicalButton::BackRight => "Back Paddle Right (R4)",
    }
}

fn action_label(act: &GameAction) -> &'static str {
    match act {
        GameAction::Jump => "Jump (Virtual A)",
        GameAction::Reload => "Reload (Virtual X)",
        GameAction::Interact => "Interact (Virtual Y)",
        GameAction::QuickSave => "Quick Save",
        GameAction::ToggleGyro => "Toggle Gyro On/Off",
        _ => "Custom Action",
    }
}

#[derive(PartialEq, Clone, Copy)]
enum UiTab {
    StickAndGyro,
    ButtonBindings,
}

#[derive(PartialEq, Clone, Copy)]
enum ActivatorKind {
    Regular,
    LongPress,
    DoubleTap,
    Chord,
}

struct MapperGuiApp {
    shared: Arc<Mutex<SharedState>>,
    active_tab: UiTab,
    // State formulir untuk menambah pemetaan baru
    new_trigger: PhysicalButton,
    new_activator_kind: ActivatorKind,
    new_duration_ms: u64,
    new_chord_modifier: PhysicalButton,
    new_action: GameAction,
}

impl MapperGuiApp {
    fn new(shared: Arc<Mutex<SharedState>>) -> Self {
        Self {
            shared,
            active_tab: UiTab::StickAndGyro,
            new_trigger: PhysicalButton::North,
            new_activator_kind: ActivatorKind::Regular,
            new_duration_ms: 400,
            new_chord_modifier: PhysicalButton::LeftBumper,
            new_action: GameAction::Interact,
        }
    }
}

impl eframe::App for MapperGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        let mut st = self.shared.lock().unwrap();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Steam-Style Input Mapper");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("🎮 {}", st.controller_name));
                });
            });
            ui.separator();

            // Navigasi Tab
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.active_tab,
                    UiTab::StickAndGyro,
                    "🎛 Stick Curve & Gyro",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    UiTab::ButtonBindings,
                    "🎮 Editor Pemetaan Tombol",
                );
            });
            ui.separator();

            match self.active_tab {
                UiTab::StickAndGyro => self.render_stick_and_gyro(ui, &mut st),
                UiTab::ButtonBindings => self.render_button_bindings(ui, &mut st),
            }
        });
    }
}

impl MapperGuiApp {
    fn render_stick_and_gyro(&self, ui: &mut egui::Ui, st: &mut SharedState) {
        ui.columns(2, |columns| {
            // KOLOM KIRI: Analog Stick & Kurva
            columns.heading("Analog Stick & Response Curve");
            columns.add_space(6.0);

            columns.add(
                egui::Slider::new(&mut st.stick_cfg.inner_deadzone, 0.0..=0.45)
                    .text("Inner Deadzone"),
            );
            columns.add(
                egui::Slider::new(&mut st.stick_cfg.outer_deadzone, 0.60..=1.0)
                    .text("Outer Deadzone"),
            );
            columns.add(
                egui::Slider::new(&mut st.stick_cfg.curve_exponent, 0.5..=3.0)
                    .text("Kurva Eksponen (1.0 = Linear)"),
            );

            columns.add_space(12.0);
            columns.label("Grafik Response Curve (Input vs Output):");

            let (response, painter) =
                columns.allocate_painter(egui::Vec2::new(220.0, 220.0), egui::Sense::hover());
            let rect = response.rect;
            painter.rect_filled(rect, 4.0, egui::Color32::from_gray(25));
            painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

            painter.line_segment(
                [rect.left_bottom(), rect.right_top()],
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            );

            let steps = 50;
            let mut points = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let input_x = (i as f32) / (steps as f32);
                let out = process_stick(input_x, 0.0, &st.stick_cfg).x;
                let px = rect.left() + input_x * rect.width();
                let py = rect.bottom() - out * rect.height();
                points.push(egui::Pos2::new(px, py));
            }
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 190, 255)),
            ));

            // KOLOM KANAN: Gyro & Visualizer
            columns.heading("Gyro & Motion Control (1€ Filter)");
            columns.add_space(6.0);

            let cal_status = if st.gyro_calibrated {
                "Terkalibrasi (Siap)"
            } else {
                "Mengkalibrasi (Diamkan kontroler...)"
            };
            columns.label(format!("Status Gyro: {}", cal_status));
            if columns.button("Kalibrasi Ulang Gyro").clicked() {
                st.recalibrate_gyro = true;
            }

            columns.add_space(6.0);
            columns.add(
                egui::Slider::new(&mut st.gyro_cfg.sensitivity_x, 1.0..=50.0)
                    .text("Sensitivitas X"),
            );
            columns.add(
                egui::Slider::new(&mut st.gyro_cfg.sensitivity_y, 1.0..=50.0)
                    .text("Sensitivitas Y"),
            );
            columns.add(
                egui::Slider::new(&mut st.gyro_cfg.min_cutoff, 0.1..=3.0)
                    .text("Smoothing (Min Cutoff)"),
            );
            columns.add(
                egui::Slider::new(&mut st.gyro_cfg.beta, 0.0..=0.15).text("Kecepatan (Beta)"),
            );
            columns.checkbox(&mut st.gyro_cfg.invert_y, "Invert Sumbu Y");

            columns.add_space(12.0);
            columns.label("Live Stick Visualizer (Merah = Raw, Hijau = Output):");

            let (stick_resp, stick_painter) =
                columns.allocate_painter(egui::Vec2::new(150.0, 150.0), egui::Sense::hover());
            let s_rect = stick_resp.rect;
            let center = s_rect.center();
            let radius = s_rect.width() * 0.45;

            stick_painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(1.5, egui::Color32::GRAY),
            );
            stick_painter.circle_stroke(
                center,
                radius * st.stick_cfg.inner_deadzone,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 100, 100)),
            );

            let raw_pos =
                center + egui::Vec2::new(st.raw_left.x * radius, -st.raw_left.y * radius);
            stick_painter.circle_filled(raw_pos, 4.0, egui::Color32::RED);

            let proc_pos = center
                + egui::Vec2::new(st.processed_left.x * radius, -st.processed_left.y * radius);
            stick_painter.circle_filled(proc_pos, 5.0, egui::Color32::GREEN);
        });
    }

    fn render_button_bindings(&mut self, ui: &mut egui::Ui, st: &mut SharedState) {
        ui.heading("Tambah Pemetaan Baru");
        ui.add_space(4.0);

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Tombol Fisik:");
                egui::ComboBox::from_id_salt("combo_trigger")
                    .selected_text(button_label(self.new_trigger))
                    .show_ui(ui, |ui| {
                        for &btn in &ALL_BUTTONS {
                            ui.selectable_value(&mut self.new_trigger, btn, button_label(btn));
                        }
                    });

                ui.label("-> Aksi Game:");
                egui::ComboBox::from_id_salt("combo_action")
                    .selected_text(action_label(&self.new_action))
                    .show_ui(ui, |ui| {
                        for act in &ALL_ACTIONS {
                            ui.selectable_value(
                                &mut self.new_action,
                                act.clone(),
                                action_label(act),
                            );
                        }
                    });
            });

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Jenis Aktivator:");
                ui.selectable_value(
                    &mut self.new_activator_kind,
                    ActivatorKind::Regular,
                    "Regular Press",);
ui.selectable_value(
&mut self.new_activator_kind,
ActivatorKind::LongPress,
"Long Press (Hold)",
);
ui.selectable_value(
&mut self.new_activator_kind,
ActivatorKind::DoubleTap,
"Double Tap",
);
ui.selectable_value(
&mut self.new_activator_kind,
ActivatorKind::Chord,
"Chord (Kombinasi)",
);
});
// Parameter tambahan sesuai jenis aktivator yang dipilih
match self.new_activator_kind {
ActivatorKind::Regular => {}
ActivatorKind::LongPress | ActivatorKind::DoubleTap => {
ui.add(
egui::Slider::new(&mut self.new_duration_ms, 100..=1500)
.text("Durasi / Jendela Waktu (ms)"),
);
}
ActivatorKind::Chord => {
ui.horizontal(|ui| {
ui.label("Harus menahan tombol Modifier:");
egui::ComboBox::from_id_salt("combo_modifier")
.selected_text(button_label(self.new_chord_modifier))
.show_ui(ui, |ui| {
for &btn in &ALL_BUTTONS {
ui.selectable_value(
&mut self.new_chord_modifier,
btn,
button_label(btn),
);
}
});
});
}
}
ui.add_space(6.0);
if ui.button("➕ Tambah ke Profil Aktif").clicked() {
let activator = match self.new_activator_kind {
ActivatorKind::Regular => ActivatorType::RegularPress,
ActivatorKind::LongPress => ActivatorType::LongPress {
hold_duration_ms: self.new_duration_ms,
},
ActivatorKind::DoubleTap => ActivatorType::DoubleTap {
window_ms: self.new_duration_ms,
},
ActivatorKind::Chord => ActivatorType::Chord {
modifier: self.new_chord_modifier,
},
};
st.bindings.push(ActionBinding {
trigger: self.new_trigger,
activator,
action: self.new_action.clone(),
});
st.bindings_dirty = true;
}
});
ui.add_space(12.0);
ui.heading("Daftar Pemetaan Aktif");
ui.add_space(4.0);
let mut index_to_remove: Option = None;
egui::ScrollArea::vertical().show(ui, |ui| {
for (idx, binding) in st.bindings.iter().enumerate() {
ui.horizontal(|ui| {
let act_desc = match &binding.activator {
ActivatorType::RegularPress => "Regular Press".to_string(),
ActivatorType::LongPress { hold_duration_ms } => {
format!("Long Press ({} ms)", hold_duration_ms)
}
ActivatorType::DoubleTap { window_ms } => {
format!("Double Tap ({} ms)", window_ms)
}
ActivatorType::Chord { modifier } => {
format!("Chord (+ {})", button_label(*modifier))
}
};
ui.label(format!(
"• [{}]  —  {}  ➡  {}",
button_label(binding.trigger),
act_desc,
action_label(&binding.action)
));
if ui.button("🗑 Hapus").clicked() {
index_to_remove = Some(idx);
}
});
ui.separator();
}
});
if let Some(idx) = index_to_remove {
st.bindings.remove(idx);
st.bindings_dirty = true;
}
}
}
fn main() -> eframe::Result<()> {
let shared_state = Arc::new(Mutex::new(SharedState::default()));
spawn_mapper_thread(Arc::clone(&shared_state));
let options = eframe::NativeOptions {
viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 480.0]),
..Default::default()
};
eframe::run_native(
"Rust Input Mapper",
options,
Box::new(|_cc| Ok(Box::new(MapperGuiApp::new(shared_state)))),
)
}
