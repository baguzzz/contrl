#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod output;

use eframe::egui;
use engine::{spawn_mapper_thread, SharedState};
use mapper_core::process_stick;
use std::sync::{Arc, Mutex};

struct MapperGuiApp {
    shared: Arc<Mutex<SharedState>>,
}

impl MapperGuiApp {
    fn new(shared: Arc<Mutex<SharedState>>) -> Self {
        Self { shared }
    }
}

impl eframe::App for MapperGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Minta repaint terus menerus agar visualizer stick berjalan mulus 60 FPS
        ctx.request_repaint();

        let mut st = self.shared.lock().unwrap();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Steam-Style Input Mapper");
            ui.label(format!("Status Perangkat: {}", st.controller_name));
            ui.separator();

            ui.columns(2, |columns| {
                // ==========================================
                // KOLOM KIRI: Pengaturan Analog Stick & Kurva
                // ==========================================
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

                // Gambar Grafik Kurva Respons secara custom dengan egui Painter
                let (response, painter) = columns.allocate_painter(
                    egui::Vec2::new(220.0, 220.0),
                    egui::Sense::hover(),
                );
                let rect = response.rect;
                painter.rect_filled(rect, 4.0, egui::Color32::from_gray(25));
                painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

                // Garis referensi linear (abu-abu tipis)
                painter.line_segment(
                    [rect.left_bottom(), rect.right_top()],
                    egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
                );

                // Gambar titik-titik kurva berdasarkan konfigurasi saat ini
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

                // ==========================================
                // KOLOM KANAN: Pengaturan Gyro & Visualizer
                // ==========================================
                columns[1].heading("Gyro & Motion Control (1€ Filter)");
                columns[1].add_space(6.0);

                let cal_status = if st.gyro_calibrated {
                    "Terkalibrasi (Siap)"
                } else {
                    "Mengkalibrasi (Diamkan kontroler...)"
                };
                columns[1].label(format!("Status Gyro: {}", cal_status));
                if columns[1].button("Kalibrasi Ulang Gyro").clicked() {
                    st.recalibrate_gyro = true;
                }

                columns[1].add_space(6.0);
                columns[1].add(
                    egui::Slider::new(&mut st.gyro_cfg.sensitivity_x, 1.0..=50.0)
                        .text("Sensitivitas X"),
                );
                columns[1].add(
                    egui::Slider::new(&mut st.gyro_cfg.sensitivity_y, 1.0..=50.0)
                        .text("Sensitivitas Y"),
                );
                columns[1].add(
                    egui::Slider::new(&mut st.gyro_cfg.min_cutoff, 0.1..=3.0)
                        .text("Smoothing (Min Cutoff)"),
                );
                columns[1].add(
                    egui::Slider::new(&mut st.gyro_cfg.beta, 0.0..=0.15)
                        .text("Kecepatan (Beta)"),
                );
                columns[1].checkbox(&mut st.gyro_cfg.invert_y, "Invert Sumbu Y");

                columns[1].add_space(12.0);
                columns[1].label("Live Stick Visualizer (Merah = Raw, Hijau = Output):");

                // Visualizer lingkaran Left Stick
                let (stick_resp, stick_painter) = columns[1].allocate_painter(
                    egui::Vec2::new(150.0, 150.0),
                    egui::Sense::hover(),
                );
                let s_rect = stick_resp.rect;
                let center = s_rect.center();
                let radius = s_rect.width() * 0.45;

                // Lingkaran batas luar & lingkaran area deadzone
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

                // Titik Raw Input (Merah)
                let raw_pos = center
                    + egui::Vec2::new(st.raw_left.x * radius, -st.raw_left.y * radius);
                stick_painter.circle_filled(raw_pos, 4.0, egui::Color32::RED);

                // Titik Processed Output (Hijau)
                let proc_pos = center
                    + egui::Vec2::new(st.processed_left.x * radius, -st.processed_left.y * radius);
                stick_painter.circle_filled(proc_pos, 5.0, egui::Color32::GREEN);
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let shared_state = Arc::new(Mutex::new(SharedState::default()));

    // Jalankan engine input 500Hz di background thread
    spawn_mapper_thread(Arc::clone(&shared_state));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([680.0, 440.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Input Mapper",
        options,
        Box::new(|_cc| Ok(Box::new(MapperGuiApp::new(shared_state)))),
    )
}
