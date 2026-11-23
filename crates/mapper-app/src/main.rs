
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod output;
mod profiles;

use eframe::egui;
use engine::{spawn_mapper_thread, SharedState};
use mapper_core::actions::{
    ActionBinding, ActivatorType, GameAction, PhysicalButton,
};
use mapper_core::process_stick;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

const ACCENT: egui::Color32 = egui::Color32::from_rgb(102, 192, 244);
const PANEL: egui::Color32 = egui::Color32::from_rgb(27, 38,  fifty());

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
    theme_applied: bool,
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
            theme_applied: false,
            new_profile_name: String::new(),
            new_trigger: PhysicalButton::North,
            new_activator_kind: ActivatorKind::Regular,
            new_duration_ms: 400,
            new_chord_modifier: PhysicalButton::LeftBumper,
            new_action: GameAction::Interact,
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let mut visuals = egui::Visuals::dark();

        visuals.window_fill = egui::Color32::from_rgb(18, 24, 32);
        visuals.panel_fill = egui::Color32::from_rgb(20, 27, 36);
        visuals.extreme_bg_color = egui::Color32::from_rgb(13, 18, 25);
        visuals.faint_bg_color = egui::Color32::from_rgb(31, 42, 54);
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke.color = egui::Color32::from_rgb(12, 20, 28);

        visuals.widgets.inactive.bg_fill =
            egui::Color32::from_rgb(37, 49, 63);
        visuals.widgets.inactive.bg_stroke.color =
            egui::Color32::from_rgb(54, 69, 85);
        visuals.widgets.hovered.bg_fill =
            egui::Color32::from_rgb(48, 68, 87);
        visuals.widgets.hovered.bg_stroke.color = ACCENT;
        visuals.widgets.active.bg_fill =
            egui::Color32::from_rgb(57, 91, 116);
        visuals.widgets.active.bg_stroke.color = ACCENT;

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(10.0, 9.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);

        ctx.set_style(style);
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui, st: &mut SharedState) {
        ui.add_space(12.0);

        ui.label(
            egui::RichText::new("CONTROLLER")
                .size(11.0)
                .strong()
                .color(egui::Color32::from_gray(145)),
        );
        ui.add_space(4.0);

        ui.label(
            egui::RichText::new("Input Mapper")
                .size(20.0)
                .strong()
                .color(egui::Color32::WHITE),
        );

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(
            egui::RichText::new("KONFIGURASI")
                .size(11.0)
                .strong()
                .color(egui::Color32::from_gray(145)),
        );

        ui.add_space(6.0);

        let stick_selected = self.active_tab == UiTab::StickAndGyro;
        if ui
            .selectable_label(stick_selected, "🎛  Stick & Gyro")
            .clicked()
        {
            self.active_tab = UiTab::StickAndGyro;
        }

        let bindings_selected = self.active_tab == UiTab::ButtonBindings;
        if ui
            .selectable_label(bindings_selected, "🎮  Button Layout")
            .clicked()
        {
            self.active_tab = UiTab::ButtonBindings;
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("PROFIL")
                .size(11.0)
                .strong()
                .color(egui::Color32::from_gray(145)),
        );

        let mut profile_names: Vec<String> =
            st.store.profiles.keys().cloned().collect();
        profile_names.sort();

        let old_profile_name = st.store.active_profile_name.clone();
        let mut selected_profile = old_profile_name.clone();

        egui::ComboBox::from_id_salt("sidebar_profile_selector")
            .width(ui.available_width())
            .selected_text(&selected_profile)
            .show_ui(ui, |ui| {
                for name in &profile_names {
                    ui.selectable_value(
                        &mut selected_profile,
                        name.clone(),
                        name,
                    );
                }
            });

        if selected_profile != old_profile_name {
            st.store.active_profile_name = selected_profile.clone();
            st.bindings_dirty = true;
            st.status_message =
                format!("Profil aktif: {}", selected_profile);
        }

        ui.add_space(8.0);

        let profile_count = st.store.profiles.len();
        ui.label(
            egui::RichText::new(format!("{profile_count} profil tersimpan"))
                .small()
                .color(egui::Color32::from_gray(150)),
        );

        ui.add_space(10.0);

        ui.add(
            egui::TextEdit::singleline(&mut self.new_profile_name)
                .hint_text("Nama profil baru")
                .desired_width(ui.available_width()),
        );

        if ui
            .add_sized(
                [ui.available_width(), 34.0],
                egui::Button::new("＋  Buat Profil"),
            )
            .clicked()
        {
            let name = self.new_profile_name.trim().to_string();

            if name.is_empty() {
                st.status_message = "Masukkan nama profil.".to_string();
            } else if st.store.profiles.contains_key(&name) {
                st.status_message =
                    format!("Profil '{}' sudah ada.", name);
            } else {
                let mut profile = st.store.active_profile().clone();
                profile.name = name.clone();
                profile.window_keyword = name.to_lowercase();

                st.store.profiles.insert(name.clone(), profile);
                st.store.active_profile_name = name.clone();
                st.bindings_dirty = true;
                self.new_profile_name.clear();
                st.status_message =
                    format!("Profil '{}' dibuat.", name);
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(
            egui::RichText::new("STATUS KONTROLER")
                .size(11.0)
                .strong()
                .color(egui::Color32::from_gray(145)),
        );

        ui.label(
            egui::RichText::new(format!("🎮 {}", st.controller_name))
                .color(egui::Color32::from_rgb(190, 210, 225)),
        );

        ui.with_layout(
            egui::Layout::bottom_up(egui::Align::Min),
            |ui| {
                ui.label(
                    egui::RichText::new("Steam Input inspired")
                        .small()
                        .color(egui::Color32::from_gray(115)),
                );
            },
        );
    }

    fn render_stick_and_gyro(
        &self,
        ui: &mut egui::Ui,
        st: &mut SharedState,
    ) {
        let raw_left = st.raw_left;
        let processed_left = st.processed_left;
        let gyro_calibrated = st.gyro_calibrated;
        let mut trigger_recalibration = false;

        ui.heading("Stick & Gyro");
        ui.label(
            egui::RichText::new(
                "Atur respons analog dan sensitivitas gyro untuk profil aktif.",
            )
            .color(egui::Color32::from_gray(165)),
        );
        ui.add_space(14.0);

        let profile_name = st.store.active_profile_name.clone();
        ui.label(
            egui::RichText::new(format!("Profil: {profile_name}"))
                .small()
                .color(ACCENT),
        );
        ui.add_space(8.0);

        let profile = st.store.active_profile_mut();

        ui.columns(2, |columns| {
            columns[0].group(|ui| {
                ui.heading("Analog Stick");
                ui.label(
                    egui::RichText::new(
                        "Sesuaikan deadzone dan kurva respons.",
                    )
                    .small()
                    .color(egui::Color32::from_gray(160)),
                );
                ui.add_space(10.0);

                ui.add(
                    egui::Slider::new(
                        &mut profile.stick_cfg.inner_deadzone,
                        0.0..=0.45,
                    )
                    .text("Inner deadzone")
                    .suffix("%")
                    .custom_formatter(|value, _| {
                        format!("{:.0}", value * 100.0)
                    })
                    .custom_parser(|text| {
                        text.parse::<f64>().ok().map(|value| value / 100.0)
                    }),
                );

                ui.add(
                    egui::Slider::new(
                        &mut profile.stick_cfg.outer_deadzone,
                        0.60..=1.0,
                    )
                    .text("Outer deadzone")
                    .custom_formatter(|value, _| {
                        format!("{:.0}%", value * 100.0)
                    })
                    .custom_parser(|text| {
                        text.trim_end_matches('%')
                            .parse::<f64>()
                            .ok()
                            .map(|value| value / 100.0)
                    }),
                );

                ui.add(
                    egui::Slider::new(
                        &mut profile.stick_cfg.curve_exponent,
                        0.5..=3.0,
                    )
                    .text("Response curve"),
                );

                ui.add_space(10.0);
                ui.label("Response curve preview");

                let (response, painter) = ui.allocate_painter(
                    egui::vec2(ui.available_width(), 190.0),
                    egui::Sense::hover(),
                );
                let rect = response.rect;

                painter.rect_filled(
                    rect,
                    5.0,
                    egui::Color32::from_rgb(15, 21, 29),
                );
                painter.rect_stroke(
                    rect,
                    5.0,
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(57, 71, 86),
                    ),
                );

                painter.line_segment(
                    [rect.left_bottom(), rect.right_top()],
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(57, 71, 86),
                    ),
                );

                let mut points = Vec::with_capacity(51);
                for i in 0..=50 {
                    let input = i as f32 / 50.0;
                    let output =
                        process_stick(input, 0.0, &profile.stick_cfg).x;

                    points.push(egui::pos2(
                        rect.left() + input * rect.width(),
                        rect.bottom() - output * rect.height(),
                    ));
                }

                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(2.5_f32, ACCENT),
                ));
            });

            columns[1].group(|ui| {
                ui.heading("Gyro");
                ui.label(
                    egui::RichText::new(
                        "Motion control dan penyaringan gerakan.",
                    )
                    .small()
                    .color(egui::Color32::from_gray(160)),
                );
                ui.add_space(10.0);

                let (status_text, status_color) = if gyro_calibrated {
                    (
                        "Terkalibrasi",
                        egui::Color32::from_rgb(128, 210, 145),
                    )
                } else {
                    (
                        "Mengkalibrasi — diamkan kontroler",
                        egui::Color32::from_rgb(245, 196, 100),
                    )
                };

                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.label(
                        egui::RichText::new(status_text)
                            .strong()
                            .color(status_color),
                    );
                });

                if ui.button("Kalibrasi ulang").clicked() {
                    trigger_recalibration = true;
                }

                ui.add_space(8.0);

                ui.add(
                    egui::Slider::new(
                        &mut profile.gyro_cfg.sensitivity_x,
                        1.0..=50.0,
                    )
                    .text("Sensitivitas horizontal"),
                );

                ui.add(
                    egui::Slider::new(
                        &mut profile.gyro_cfg.sensitivity_y,
                        1.0..=50.0,
                    )
                    .text("Sensitivitas vertikal"),
                );

                ui.add(
                    egui::Slider::new(
                        &mut profile.gyro_cfg.min_cutoff,
                        0.1..=3.0,
                    )
                    .text("Smoothing"),
                );

                ui.add(
                    egui::Slider::new(
                        &mut profile.gyro_cfg.beta,
                        0.0..=0.15,
                    )
                    .text("Respons gerakan cepat"),
                );

                ui.checkbox(
                    &mut profile.gyro_cfg.invert_y,
                    "Balikkan sumbu vertikal",
                );

                ui.add_space(8.0);
                ui.label("Live stick preview");

                let (response, painter) =
                    ui.allocate_painter(egui::vec2(190.0, 190.0), egui::Sense::hover());
                let rect = response.rect;
                let center = rect.center();
                let radius = rect.width().min(rect.height()) * 0.43;

                painter.circle_filled(
                    center,
                    radius,
                    egui::Color32::from_rgb(15, 21, 29),
                );
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(70, 87, 103),
                    ),
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x - radius, center.y),
                        egui::pos2(center.x + radius, center.y),
                    ],
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(48, 61, 75),
                    ),
                );
                painter.line_segment(
                    [
                        egui::pos2(center.x, center.y - radius),
                        egui::pos2(center.x, center.y + radius),
                    ],
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(48, 61, 75),
                    ),
                );
                painter.circle_stroke(
                    center,
                    radius * profile.stick_cfg.inner_deadzone,
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(220, 100, 100),
                    ),
                );

                let raw_pos = center
                    + egui::vec2(raw_left.x, -raw_left.y) * radius;
                painter.circle_filled(
                    raw_pos,
                    4.0,
                    egui::Color32::from_rgb(255, 100, 100),
                );

                let processed_pos = center
                    + egui::vec2(processed_left.x, -processed_left.y)
                        * radius;
                painter.circle_filled(
                    processed_pos,
                    5.0,
                    egui::Color32::from_rgb(130, 220, 150),
                );

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("● Raw")
                            .small()
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                    ui.label(
                        egui::RichText::new("● Processed")
                            .small()
                            .color(egui::Color32::from_rgb(130, 220, 150)),
                    );
                });
            });
        });

        if trigger_recalibration {
            st.recalibrate_gyro = true;
        }
    }

    fn render_button_bindings(
        &mut self,
        ui: &mut egui::Ui,
        st: &mut SharedState,
    ) {
        ui.heading("Button Layout");
        ui.label(
            egui::RichText::new(
                "Pilih tombol fisik, aktivator, dan aksi yang ingin dijalankan.",
            )
            .color(egui::Color32::from_gray(165)),
        );
        ui.add_space(14.0);

        let mut new_binding: Option<ActionBinding> = None;

        ui.group(|ui| {
            ui.heading("Tambah binding");
            ui.add_space(8.0);

            egui::Grid::new("binding_editor_grid")
                .num_columns(2)
                .spacing([14.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Tombol");
                    egui::ComboBox::from_id_salt("combo_trigger")
                        .selected_text(button_label(self.new_trigger))
                        .show_ui(ui, |ui| {
                            for &button in &ALL_BUTTONS {
                                ui.selectable_value(
                                    &mut self.new_trigger,
                                    button,
                                    button_label(button),
                                );
                            }
                        });
                    ui.end_row();

                    ui.label("Aksi");
                    egui::ComboBox::from_id_salt("combo_action")
                        .selected_text(action_label(&self.new_action))
                        .show_ui(ui, |ui| {
                            for action in &ALL_ACTIONS {
                                ui.selectable_value(
                                    &mut self.new_action,
                                    action.clone(),
                                    action_label(action),
                                );
                            }
                        });
                    ui.end_row();

                    ui.label("Aktivator");
                    egui::ComboBox::from_id_salt("combo_activator")
                        .selected_text(activator_label(
                            self.new_activator_kind,
                        ))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.new_activator_kind,
                                ActivatorKind::Regular,
                                "Regular press",
                            );
                            ui.selectable_value(
                                &mut self.new_activator_kind,
                                ActivatorKind::LongPress,
                                "Long press",
                            );
                            ui.selectable_value(
                                &mut self.new_activator_kind,
                                ActivatorKind::DoubleTap,
                                "Double tap",
                            );
                            ui.selectable_value(
                                &mut self.new_activator_kind,
                                ActivatorKind::Chord,
                                "Chord",
                            );
                        });
                    ui.end_row();

                    match self.new_activator_kind {
                        ActivatorKind::LongPress
                        | ActivatorKind::DoubleTap => {
                            ui.label(match self.new_activator_kind {
                                ActivatorKind::LongPress => "Durasi tahan",
                                _ => "Jendela double tap",
                            });
                            ui.add(
                                egui::Slider::new(
                                    &mut self.new_duration_ms,
                                    100..=1500,
                                )
                                .suffix(" ms"),
                            );
                            ui.end_row();
                        }
                        ActivatorKind::Chord => {
                            ui.label("Tombol modifier");
                            egui::ComboBox::from_id_salt("combo_modifier")
                                .selected_text(button_label(
                                    self.new_chord_modifier,
                                ))
                                .show_ui(ui, |ui| {
                                    for &button in &ALL_BUTTONS {
                                        ui.selectable_value(
                                            &mut self.new_chord_modifier,
                                            button,
                                            button_label(button),
                                        );
                                    }
                                });
                            ui.end_row();
                        }
                        ActivatorKind::Regular => {}
                    }
                });

            ui.add_space(10.0);

            if ui
                .add_sized(
                    [ui.available_width(), 36.0],
                    egui::Button::new("＋  Tambahkan binding"),
                )
                .clicked()
            {
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

                new_binding = Some(ActionBinding {
                    trigger: self.new_trigger,
                    activator,
                    action: self.new_action.clone(),
                });
            }
        });

        if let Some(binding) = new_binding {
            st.store
                .active_profile_mut()
                .bindings
                .push(binding);
            st.bindings_dirty = true;
            st.status_message = "Binding ditambahkan.".to_string();
        }

        ui.add_space(16.0);

        let active_profile = st.store.active_profile_name.clone();
        ui.horizontal(|ui| {
            ui.heading("Binding aktif");
            ui.label(
                egui::RichText::new(format!("Profil: {active_profile}"))
                    .small()
                    .color(ACCENT),
            );
        });
        ui.add_space(6.0);

        let bindings = st.store.active_profile().bindings.clone();
        let mut remove_index: Option<usize> = None;

        egui::ScrollArea::vertical()
            .max_height(ui.available_height())
            .show(ui, |ui| {
                if bindings.is_empty() {
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "Belum ada binding. Tambahkan binding di atas.",
                            )
                            .color(egui::Color32::from_gray(160)),
                        );
                    });
                }

                for (index, binding) in bindings.iter().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(button_label(
                                        binding.trigger,
                                    ))
                                    .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  →  {}",
                                        binding_activator_label(
                                            &binding.activator
                                        ),
                                        action_label(&binding.action),
                                    ))
                                    .small()
                                    .color(
                                        egui::Color32::from_gray(165),
                                    ),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(
                                    egui::Align::Center,
                                ),
                                |ui| {
                                    if ui.small_button("Hapus").clicked() {
                                        remove_index = Some(index);
                                    }
                                },
                            );
                        });
                    });
                }
            });

        if let Some(index) = remove_index {
            st.store
                .active_profile_mut()
                .bindings
                .remove(index);
            st.bindings_dirty = true;
            st.status_message = "Binding dihapus.".to_string();
        }
    }
}

impl eframe::App for MapperGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            Self::apply_theme(ctx);
            self.theme_applied = true;
        }

        ctx.request_repaint_after(Duration::from_millis(33));

        let mut st = match self.shared.lock() {
            Ok(state) => state,
            Err(_) => return,
        };

        egui::TopBottomPanel::top("top_bar")
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("INPUT CONFIGURATION")
                            .size(11.0)
                            .strong()
                            .color(egui::Color32::from_gray(145)),
                    );

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.button("Simpan profil").clicked() {
                                match st.store.save() {
                                    Ok(()) => {
                                        st.status_message =
                                            "Profil tersimpan.".to_string();
                                    }
                                    Err(error) => {
                                        st.status_message =
                                            format!("Gagal menyimpan: {error}");
                                    }
                                }
                            }

                            ui.checkbox(
                                &mut st.auto_switch_enabled,
                                "Auto-switch",
                            );

                            ui.separator();
                            ui.label(format!("🎮 {}", st.controller_name));
                        },
                    );
                });
            });

        egui::SidePanel::left("navigation_sidebar")
            .resizable(false)
            .exact_width(210.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui, &mut st);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Jendela aktif:")
                        .small()
                        .color(egui::Color32::from_gray(145)),
                );
                ui.label(
                    egui::RichText::new(&st.active_window_title)
                        .small()
                        .color(egui::Color32::from_gray(195)),
                );
            });

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.active_tab {
                    UiTab::StickAndGyro => {
                        self.render_stick_and_gyro(ui, &mut st);
                    }
                    UiTab::ButtonBindings => {
                        self.render_button_bindings(ui, &mut st);
                    }
                });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("STATUS")
                        .small()
                        .strong()
                        .color(ACCENT),
                );
                ui.label(
                    egui::RichText::new(&st.status_message)
                        .small()
                        .color(egui::Color32::from_gray(185)),
                );
            });
        });
    }
}

fn button_label(button: PhysicalButton) -> &'static str {
    match button {
        PhysicalButton::South => "South (A / Cross)",
        PhysicalButton::East => "East (B / Circle)",
        PhysicalButton::West => "West (X / Square)",
        PhysicalButton::North => "North (Y / Triangle)",
        PhysicalButton::LeftBumper => "Left bumper (LB / L1)",
        PhysicalButton::RightBumper => "Right bumper (RB / R1)",
        PhysicalButton::LeftTriggerBtn => "Left trigger (LT / L2)",
        PhysicalButton::RightTriggerBtn => "Right trigger (RT / R2)",
        PhysicalButton::BackLeft => "Back paddle left",
        PhysicalButton::BackRight => "Back paddle right",
    }
}

fn action_label(action: &GameAction) -> &'static str {
    match action {
        GameAction::Jump => "Jump",
        GameAction::Reload => "Reload",
        GameAction::Interact => "Interact",
        GameAction::QuickSave => "Quick Save",
        GameAction::ToggleGyro => "Toggle Gyro",
        _ => "Custom action",
    }
}

fn activator_label(activator: ActivatorKind) -> &'static str {
    match activator {
        ActivatorKind::Regular => "Regular press",
        ActivatorKind::LongPress => "Long press",
        ActivatorKind::DoubleTap => "Double tap",
        ActivatorKind::Chord => "Chord",
    }
}

fn binding_activator_label(activator: &ActivatorType) -> String {
    match activator {
        ActivatorType::RegularPress => "Regular press".to_string(),
        ActivatorType::LongPress { hold_duration_ms } => {
            format!("Long press · {hold_duration_ms} ms")
        }
        ActivatorType::DoubleTap { window_ms } => {
            format!("Double tap · {window_ms} ms")
        }
        ActivatorType::Chord { modifier } => {
            format!("Chord + {}", button_label(*modifier))
        }
    }
}

fn main() -> eframe::Result<()> {
    let shared_state = Arc::new(Mutex::new(SharedState::default()));
    spawn_mapper_thread(Arc::clone(&shared_state));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([800.0, 560.0]),
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "Input Mapper",
        options,
        Box::new(|_creation_context| {
            Ok(Box::new(MapperGuiApp::new(shared_state)))
        }),
    )
}