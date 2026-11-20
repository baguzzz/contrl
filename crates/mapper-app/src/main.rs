mod output;

use glam::{Vec2, Vec3};
use mapper_core::actions::{
    ActionBinding, ActionEngine, ActionSet, ActivatorType, GameAction, PhysicalButton,
};
use mapper_core::gyro::{GyroConfig, GyroProcessor};
use mapper_core::{process_stick, StickConfig};
use output::{create_platform_output, OutputState};
use sdl3::gamepad::{Axis, Button};
use sdl3::sensor::SensorType;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

fn map_sdl_button(btn: Button) -> Option<PhysicalButton> {
    match btn {
        Button::South => Some(PhysicalButton::South),
        Button::East => Some(PhysicalButton::East),
        Button::West => Some(PhysicalButton::West),
        Button::North => Some(PhysicalButton::North),
        Button::LeftShoulder => Some(PhysicalButton::LeftBumper),
        Button::RightShoulder => Some(PhysicalButton::RightBumper),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Inisialisasi SDL3 dengan dukungan Gamepad & Sensor (Gyro)
    let sdl = sdl3::init()?;
    let gamepad_sys = sdl.gamepad()?;
    let mut event_pump = sdl.event_pump()?;

    // 2. Siapkan konfigurasi DSP & Profil Default
    let stick_cfg = StickConfig {
        inner_deadzone: 0.12,
        outer_deadzone: 0.95,
        curve_exponent: 1.5,
    };

    let mut gyro_processor = GyroProcessor::new(GyroConfig {
        sensitivity_x: 18.0, // Skala ke piksel mouse
        sensitivity_y: 18.0,
        min_cutoff: 1.0,
        beta: 0.03,
        invert_y: false,
    });

    let default_set = ActionSet {
        name: "Default".to_string(),
        bindings: vec![
            ActionBinding {
                trigger: PhysicalButton::South,
                activator: ActivatorType::RegularPress,
                action: GameAction::Jump,
            },
            ActionBinding {
                trigger: PhysicalButton::West,
                activator: ActivatorType::LongPress { hold_duration_ms: 400 },
                action: GameAction::Reload,
            },
        ],
        layers: HashMap::new(),
    };
    let mut action_engine = ActionEngine::new(default_set);

    // 3. Inisialisasi Output Virtual (ViGEm di Windows)
    let mut virtual_out = create_platform_output();

    // State pelacakan kontroler aktif
    let mut active_gamepad: Option<sdl3::gamepad::Gamepad> = None;
    let mut pressed_buttons: HashSet<PhysicalButton> = HashSet::new();
    let mut raw_left_stick = Vec2::ZERO;
    let mut raw_right_stick = Vec2::ZERO;
    let mut latest_gyro = Vec3::ZERO;

    let mut last_tick = Instant::now();
    let target_frame_time = Duration::from_millis(2); // 500Hz polling loop

    println!("Menunggu kontroler terhubung... (Tekan Ctrl+C untuk keluar)");

    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32();
        last_tick = now;

        // 4. Baca semua event hardware dari SDL3
        for event in event_pump.poll_iter() {
            use sdl3::event::Event;
            match event {
                Event::GamepadAdded { which } => {
                    if let Ok(mut gp) = gamepad_sys.open(which) {
                        println!("Kontroler terhubung: {}", gp.name().unwrap_or_default());
                        // Aktifkan sensor Gyro jika tersedia (DualSense, DualShock 4, Switch Pro)
                        if gp.has_sensor(SensorType::Gyro) {
                            let _ = gp.set_sensor_enabled(SensorType::Gyro, true);
                            println!("Sensor Gyro aktif! Diamkan kontroler 1 detik untuk kalibrasi...");
                            gyro_processor.calibrator.reset();
                        }
                        active_gamepad = Some(gp);
                    }
                }
                Event::GamepadButtonDown { button, .. } => {
                    if let Some(pb) = map_sdl_button(button) {
                        pressed_buttons.insert(pb);
                    }
                }
                Event::GamepadButtonUp { button, .. } => {
                    if let Some(pb) = map_sdl_button(button) {
                        pressed_buttons.remove(&pb);
                    }
                }
                Event::GamepadAxisMotion { axis, value, .. } => {
                    let norm = (value as f32) / 32768.0;
                    match axis {
                        Axis::LeftX => raw_left_stick.x = norm,
                        Axis::LeftY => raw_left_stick.y = -norm, // Invert Y agar atas = +1.0
                        Axis::RightX => raw_right_stick.x = norm,
                        Axis::RightY => raw_right_stick.y = -norm,
                        _ => {}
                    }
                }
                Event::GamepadSensorUpdate { sensor, data, .. } => {
                    if sensor == SensorType::Gyro && data.len() >= 3 {
                        latest_gyro = Vec3::new(data[0], data[1], data[2]);
                    }
                }
                Event::Quit { .. } => return Ok(()),
                _ => {}
            }
        }

        // 5. Jalankan Pipeline Pemrosesan (jika kontroler terpasang)
        if active_gamepad.is_some() {
            let processed_l = process_stick(raw_left_stick.x, raw_left_stick.y, &stick_cfg);
            let processed_r = process_stick(raw_right_stick.x, raw_right_stick.y, &stick_cfg);
            let gyro_delta = gyro_processor.process(latest_gyro, dt);
            let triggered_actions: HashSet<GameAction> = action_engine
                .update(&pressed_buttons, now)
                .into_iter()
                .collect();

            // 6. Kirim hasil akhir ke kontroler virtual & mouse
            virtual_out.send(&OutputState {
                left_stick: processed_l,
                right_stick: processed_r,
                gyro_mouse_delta: gyro_delta,
                actions: triggered_actions,
            });
        }

        // Jaga agar loop berjalan stabil di ~500Hz tanpa memakan 100% CPU
        let elapsed = now.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
    }
}
