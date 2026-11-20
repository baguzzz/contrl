use crate::output::{create_platform_output, OutputState};
use glam::{Vec2, Vec3};
use mapper_core::actions::{
    ActionBinding, ActionEngine, ActionSet, ActivatorType, GameAction, PhysicalButton,
};
use mapper_core::gyro::{GyroConfig, GyroProcessor};
use mapper_core::{process_stick, StickConfig};
use sdl3::gamepad::{Axis, Button};
use sdl3::sensor::SensorType;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct SharedState {
    pub stick_cfg: StickConfig,
    pub gyro_cfg: GyroConfig,
    pub recalibrate_gyro: bool,
    // Telemetri untuk ditampilkan di UI
    pub controller_name: String,
    pub raw_left: Vec2,
    pub processed_left: Vec2,
    pub gyro_calibrated: bool,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            stick_cfg: StickConfig {
                inner_deadzone: 0.12,
                outer_deadzone: 0.95,
                curve_exponent: 1.5,
            },
            gyro_cfg: GyroConfig {
                sensitivity_x: 18.0,
                sensitivity_y: 18.0,
                min_cutoff: 1.0,
                beta: 0.03,
                invert_y: false,
            },
            recalibrate_gyro: false,
            controller_name: "Menunggu kontroler...".to_string(),
            raw_left: Vec2::ZERO,
            processed_left: Vec2::ZERO,
            gyro_calibrated: false,
        }
    }
}

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

pub fn spawn_mapper_thread(shared: Arc<Mutex<SharedState>>) {
    std::thread::spawn(move || {
        let sdl = match sdl3::init() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Gagal inisialisasi SDL3: {e}");
                return;
            }
        };
        let gamepad_sys = sdl.gamepad().unwrap();
        let mut event_pump = sdl.event_pump().unwrap();

        let initial_gyro_cfg = shared.lock().unwrap().gyro_cfg;
        let mut gyro_processor = GyroProcessor::new(initial_gyro_cfg);

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
        let mut virtual_out = create_platform_output();

        let mut active_gamepad: Option<sdl3::gamepad::Gamepad> = None;
        let mut pressed_buttons: HashSet<PhysicalButton> = HashSet::new();
        let mut raw_left_stick = Vec2::ZERO;
        let mut raw_right_stick = Vec2::ZERO;
        let mut latest_gyro = Vec3::ZERO;

        let mut last_tick = Instant::now();
        let target_frame_time = Duration::from_millis(2); // 500Hz

        loop {
            let now = Instant::now();
            let dt = now.duration_since(last_tick).as_secs_f32();
            last_tick = now;

            for event in event_pump.poll_iter() {
                use sdl3::event::Event;
                match event {
                    Event::GamepadAdded { which } => {
                        if let Ok(mut gp) = gamepad_sys.open(which) {
                            let name = gp.name().unwrap_or_else(|_| "Unknown Gamepad".into());
                            if gp.has_sensor(SensorType::Gyro) {
                                let _ = gp.set_sensor_enabled(SensorType::Gyro, true);
                                gyro_processor.calibrator.reset();
                            }
                            if let Ok(mut st) = shared.lock() {
                                st.controller_name = name;
                            }
                            active_gamepad = Some(gp);
                        }
                    }
                    Event::GamepadRemoved { .. } => {
                        active_gamepad = None;
                        if let Ok(mut st) = shared.lock() {
                            st.controller_name = "Kontroler terputus".to_string();
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
                            Axis::LeftY => raw_left_stick.y = -norm,
                            Axis::RightX => raw_right_stick.x = norm,
                            Axis::RightY => raw_right_stick.y = -norm,
                            _ => {}
                        }
                    }
                    Event::GamepadSensorUpdate { sensor, data, .. } => {
                        if sensor == SensorType::Gyro && data.len() >= 3 {
                            latest_gyro = Vec3::new(data, data, data);
                        }
                    }
                    _ => {}
                }
            }

            // Ambil konfigurasi terbaru dari GUI
            let (stick_cfg, gyro_cfg, need_recal) = {
                let mut st = shared.lock().unwrap();
                let recal = st.recalibrate_gyro;
                st.recalibrate_gyro = false;
                (st.stick_cfg, st.gyro_cfg, recal)
            };

            if need_recal {
                gyro_processor.calibrator.reset();
            }
            gyro_processor.config = gyro_cfg;

            if active_gamepad.is_some() {
                let processed_l = process_stick(raw_left_stick.x, raw_left_stick.y, &stick_cfg);
                let processed_r = process_stick(raw_right_stick.x, raw_right_stick.y, &stick_cfg);
                let gyro_delta = gyro_processor.process(latest_gyro, dt);
                let triggered_actions: HashSet<GameAction> = action_engine
                    .update(&pressed_buttons, now)
                    .into_iter()
                    .collect();

                virtual_out.send(&OutputState {
                    left_stick: processed_l,
                    right_stick: processed_r,
                    gyro_mouse_delta: gyro_delta,
                    actions: triggered_actions,
                });

                // Kirim data telemetri balik ke GUI
                if let Ok(mut st) = shared.lock() {
                    st.raw_left = raw_left_stick;
                    st.processed_left = processed_l;
                    st.gyro_calibrated = gyro_processor.calibrator.is_calibrated;
                }
            }

            let elapsed = now.elapsed();
            if elapsed < target_frame_time {
                std::thread::sleep(target_frame_time - elapsed);
            }
        }
    });
}
