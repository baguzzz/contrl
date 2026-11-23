#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod output;
mod profiles;

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
    new_profile_name: String,
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
            new_profile_name: String::new(),
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
        let shared = Arc::clone(&self.shared);

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut st = shared.lock().unwrap();

            ui.horizontal(|ui| {
                ui.heading("Steam-Style Input Mapper");
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(format!("🎮 {}", st.controller_name));
                    },
                );
            });
            ui.separator();

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Profil Aktif:");

                    let mut profile_names: Vec<String> =
                        st.store.profiles.keys().cloned().collect();
                    profile_names.sort();

                    let current_name = st.store.active_profile_name.clone();

                    egui::ComboBox::from_id_salt("combo_profile")
                        .selected_text(&current_name)
                        .show_ui(ui, |ui| {
                            for name in profile_names {
                                if ui
                                    .selectable_value(
                                        &mut st.store.active_profile_name,
                                        name.clone(),
                                        &name,
                                    )
                                    .clicked()
                                {
                                    st.bindings_dirty = true;
                                    st.status_message =
                                        format!("Berpindah ke profil: {}", name);
                                }
                            }
                        });

                    if ui.button("💾 Simpan JSON").clicked() {
                        match st.store.save() {
                            Ok(_) => {
                                st.status_message =
                                    "Tersimpan ke profiles.json!".to_string()
                            }
                            Err(e) => {
                                st.status_message = format!("Gagal simpan: {}", e)
                            }
                        }
                    }

                    ui.checkbox(&mut st.auto_switch_enabled, "Auto-Switch Game");
                });

                ui.horizontal(|ui| {
                    ui.label("Kata Kunci Jendela Game:");
                    let prof = st.store.active_profile_mut();

                    ui.add(
                        egui::TextEdit::singleline(&mut prof.window_keyword)
                            .hint_text("misal: valorant, cs2, cyberpunk")
                            .desired_width(180.0),
                    );

                    ui.separator();
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_profile_name)
                            .hint_text("Nama profil baru...")
                            .desired_width(130.0),
                    );

                    if ui.button("➕ Buat Profil").clicked() {
                        let trimmed = self.new_profile_name.trim().to_string();

                        if !trimmed.is_empty()
                            && !st.store.profiles.contains_key(&trimmed)
                        {
                            let mut cloned =
                                st.store.active_profile().clone();

                            cloned.name = trimmed.clone();
                            cloned.window_keyword = trimmed.to_lowercase();

                            st.store.profiles.insert(trimmed.clone(), cloned);
                            st.store.active_profile_name = trimmed.clone();
                            st.bindings_dirty = true;
                            self.new_profile_name.clear();

                            st.status_message =
                                format!("Profil '{}' dibuat!", trimmed);
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.small(format!(
                        "Jendela Fokus: {}",
                        st.active_window_title
                    ));

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.small(
                                egui::RichText::new(&st.status_message)
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                        },
                    );
                });
            });

            ui.add_space(4.0);

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
                UiTab::StickAndGyro =>
                    self.render_stick_and_gyro(ui, &mut st),
                UiTab::ButtonBindings =>
                    self.render_button_bindings(ui, &mut st),
            }
        });
    }
}

impl MapperGuiApp {
    fn render_stick_and_gyro(
        &self,
        ui: &mut egui::Ui,
        st: &mut SharedState,
    ) {
        let raw_left = st.raw_left;
        let processed_left = st.processed_left;
        let gyro_calibrated = st.gyro_calibrated;
        let mut trigger_recal = false;

        {
            let prof = st.store.active_profile_mut();

            ui.columns(2, |columns| {
                columns[0].heading("Analog Stick & Response Curve");
                columns[0].add_space(6.0);

                columns[0].add(
                    egui::Slider::new(
                        &mut prof.stick_cfg.inner_deadzone,
                        0.0..=0.45,
                    )
                    .text("Inner Deadzone"),
                );

                columns[0].add(
                    egui::Slider::new(
                        &mut prof.stick_cfg.outer_deadzone,
                        0.60..=1.0,
                    )
                    .text("Outer Deadzone"),
                );

                columns[0].add(
                    egui::Slider::new(
                        &mut prof.stick_cfg.curve_exponent,
                        0.5..=3.0,
                    )
                    .text("Kurva Eksponen (1.0 = Linear)"),
                );

                columns[0].add_space(12.0);
                columns[0].label(
                    "Grafik Response Curve (Input vs Output):",
                );

                let (response, painter) = columns[0].allocate_painter(
                    egui::Vec2::new(210.0, 210.0),
                    egui::Sense::hover(),
                );

                let rect = response.rect;

                painter.rect_filled(
                    rect,
                    4.0,
                    egui::Color32::from_gray(25),
                );

                painter.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::GRAY,
                    ),
                );

                painter.line_segment(
                    [rect.left_bottom(), rect.right_top()],
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_gray(70),
                    ),
                );

                let steps = 50;
                let mut points = Vec::with_capacity(steps + 1);

                for i in 0..=steps {
                    let input_x = i as f32 / steps as f32;
                    let out =
                        process_stick(input_x, 0.0, &prof.stick_cfg).x;

                    let px = rect.left() + input_x * rect.width();
                    let py = rect.bottom() - out * rect.height();

                    points.push(egui::Pos2::new(px, py));
                }

                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(
                        2.5_f32,
                        egui::Color32::from_rgb(0, 190, 255),
                    ),
                ));

                columns[1].heading(
                    "Gyro & Motion Control (1€ Filter)",
                );
                columns[1].add_space(6.0);

                let cal_status = if gyro_calibrated {
                    "Terkalibrasi (Siap)"
                } else {
                    "Mengkalibrasi (Diamkan kontroler...)"
                };

                columns[1].label(format!(
                    "Status Gyro: {}",
                    cal_status
                ));

                if columns[1]
                    .button("Kalibrasi Ulang Gyro")
                    .clicked()
                {
                    trigger_recal = true;
                }

                columns[1].add_space(6.0);

                columns[1].add(
                    egui::Slider::new(
                        &mut prof.gyro_cfg.sensitivity_x,
                        1.0..=50.0,
                    )
                    .text("Sensitivitas X"),
                );

                columns[1].add(
                    egui::Slider::new(
                        &mut prof.gyro_cfg.sensitivity_y,
                        1.0..=50.0,
                    )
                    .text("Sensitivitas Y"),
                );

                columns[1].add(
                    egui::Slider::new(
                        &mut prof.gyro_cfg.min_cutoff,
                        0.1..=3.0,
                    )
                    .text("Smoothing (Min Cutoff)"),
                );

                columns[1].add(
                    egui::Slider::new(
                        &mut prof.gyro_cfg.beta,
                        0.0..=0.15,
                    )
                    .text("Kecepatan (Beta)"),
                );

                columns[1].checkbox(
                    &mut prof.gyro_cfg.invert_y,
                    "Invert Sumbu Y",
                );

                columns[1].add_space(12.0);

                columns[1].label(
                    "Live Stick Visualizer (Merah = Raw, Hijau = Output):",
                );

                let (stick_resp, stick_painter) =
                    columns[1].allocate_painter(
                        egui::Vec2::new(140.0, 140.0),
                        egui::Sense::hover(),
                    );

                let s_rect = stick_resp.rect;
                let center = s_rect.center();
                let radius = s_rect.width() * 0.45;

                stick_painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(
                        1.5_f32,
                        egui::Color32::GRAY,
                    ),
                );

                stick_painter.circle_stroke(
                    center,
                    radius * prof.stick_cfg.inner_deadzone,
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(255, 100, 100),
                    ),
                );

                let raw_pos = center
                    + egui::Vec2::new(
                        raw_left.x * radius,
                        -raw_left.y * radius,
                    );

                stick_painter.circle_filled(
                    raw_pos,
                    4.0,
                    egui::Color32::RED,
                );

                let proc_pos = center
                    + egui::Vec2::new(
                        processed_left.x * radius,
                        -processed_left.y * radius,
                    );

                stick_painter.circle_filled(
                    proc_pos,
                    5.0,
                    egui::Color32::GREEN,
                );
            });
        }

        if trigger_recal {
            st.recalibrate_gyro = true;
        }
    }

    fn render_button_bindings(
        &mut self,
        ui: &mut egui::Ui,
        st: &mut SharedState,
    ) {
        ui.heading("Tambah Pemetaan Baru");
        ui.add_space(4.0);

        let mut added_binding: Option<ActionBinding> = None;

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Tombol Fisik:");

                egui::ComboBox::from_id_salt("combo_trigger")
                    .selected_text(button_label(self.new_trigger))
                    .show_ui(ui, |ui| {
                        for &btn in &ALL_BUTTONS {
                            ui.selectable_value(
                                &mut self.new_trigger,
                                btn,
                                button_label(btn),
                            );
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
                    "Regular Press",
                );

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

            match self.new_activator_kind {
                ActivatorKind::Regular => {}

                ActivatorKind::LongPress
                | ActivatorKind::DoubleTap => {
                    ui.add(
                        egui::Slider::new(
                            &mut self.new_duration_ms,
                            100..=1500,
                        )
                        .text("Durasi / Jendela Waktu (ms)"),
                    );
                }

                ActivatorKind::Chord => {
                    ui.horizontal(|ui| {
                        ui.label("Harus menahan tombol Modifier:");

                        egui::ComboBox::from_id_salt("combo_modifier")
                            .selected_text(button_label(
                                self.new_chord_modifier,
                            ))
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
                let activator =
                    match self.new_activator_kind {
                        ActivatorKind::Regular =>
                            ActivatorType::RegularPress,

                        ActivatorKind::LongPress =>
                            ActivatorType::LongPress {
                                hold_duration_ms:
                                    self.new_duration_ms,
                            },

                        ActivatorKind::DoubleTap =>
                            ActivatorType::DoubleTap {
                                window_ms: self.new_duration_ms,
                            },

                        ActivatorKind::Chord =>
                            ActivatorType::Chord {
                                modifier:
                                    self.new_chord_modifier,
                            },
                    };

                added_binding = Some(ActionBinding {
                    trigger: self.new_trigger,
                    activator,
                    action: self.new_action.clone(),
                });
            }
        });

        if let Some(binding) = added_binding {
            st.store
                .active_profile_mut()
                .bindings
                .push(binding);

            st.bindings_dirty = true;
        }

        ui.add_space(12.0);
        ui.heading("Daftar Pemetaan Aktif");
        ui.add_space(4.0);

        let mut index_to_remove: Option<usize> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let bindings =
                &st.store.active_profile().bindings;

            for (idx, binding) in bindings.iter().enumerate() {
                ui.horizontal(|ui| {
                    let activator_description =
                        match &binding.activator {
                            ActivatorType::RegularPress =>
                                "Regular Press".to_string(),

                            ActivatorType::LongPress {
                                hold_duration_ms,
                            } =>
                                format!(
                                    "Long Press ({} ms)",
                                    hold_duration_ms
                                ),

                            ActivatorType::DoubleTap {
                                window_ms,
                            } =>
                                format!(
                                    "Double Tap ({} ms)",
                                    window_ms
                                ),

                            ActivatorType::Chord {
                                modifier,
                            } =>
                                format!(
                                    "Chord (+ {})",
                                    button_label(*modifier)
                                ),
                        };

                    ui.label(format!(
                        "• [{}] — {} ➡ {}",
                        button_label(binding.trigger),
                        activator_description,
                        action_label(&binding.action),
                    ));

                    if ui.button("🗑 Hapus").clicked() {
                        index_to_remove = Some(idx);
                    }
                });

                ui.separator();
            }
        });

        if let Some(idx) = index_to_remove {
            st.store
                .active_profile_mut()
                .bindings
                .remove(idx);

            st.bindings_dirty = true;
        }
    }
}

fn main() -> eframe::Result<()> {
    let shared_state =
        Arc::new(Mutex::new(SharedState::default()));

    spawn_mapper_thread(Arc::clone(&shared_state));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([740.0, 540.0]),
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "Rust Input Mapper",
        options,
        Box::new(|_cc|
            Ok(Box::new(MapperGuiApp::new(shared_state)))
        ),
    )
}