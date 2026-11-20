use crate::output::{create_platform_output, OutputState};
use crate::profiles::ProfileStore;
use glam::{Vec2, Vec3};
use mapper_core::actions::{ActionEngine, ActionSet};
use mapper_core::gyro::GyroProcessor;
use mapper_core::process_stick;
use sdl3::gamepad::{Axis, Button};
use sdl3::sensor::SensorType;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct SharedState {
    pub store: ProfileStore,
    pub auto_switch_enabled: bool,
    pub active_window_title: String,
    pub recalibrate_gyro: bool,
    pub bindings_dirty: bool,
    pub status_message: String,
    // Telemetri
    pub controller_name: String,
    pub raw_left: Vec2,
    pub processed_left: Vec2,
    pub gyro_calibrated: bool,
}

impl Default for SharedState {
    fn default() -> Self {
        let store = ProfileStore::load_or_default();
        Self {
            store,
            auto_switch_enabled: true,
            active_window_title: "-".to_string(),
            recalibrate_gyro: false,
            bindings_dirty: false,
            status_message: "Profil dimuat dari profiles.json".to_string(),
            controller_name: "Menunggu kontroler...".to_string(),
            raw_left: Vec2::ZERO,
            processed_left: Vec2::ZERO,
            gyro_calibrated: false,
        }
    }
}

fn map_sdl_button(btn: Button) -> Option<mapper_core::actions::PhysicalButton> {
    use mapper_core::actions::PhysicalButton;
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
    // Thread 1: Context Watcher (Auto-Switch Profil per Game setiap 300ms)
    let shared_watcher = Arc::clone(&shared);
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(300));

        if let Ok(win) = active_win_pos_rs::get_active_window() {
            let info = format!("{} | {}", win.app_name, win.title).to_lowercase();

            if let Ok(mut st) = shared_watcher.lock() {
                st.active_window_title = format!("{} ({})", win.app_name, win.title);

                if st.auto_switch_enabled {
                    // Cari profil dengan window_keyword yang cocok
                    let matched_name = st
                        .store
                        .profiles
                        .values()
                        .find(|p| {
                            !p.window_keyword.trim().is_empty()
                                && info.contains(&p.window_keyword.trim().to_lowercase())
                        })
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Default".to_string());

                    if st.store.active_profile_name != matched_name
                        && st.store.profiles.contains_key(&matched_name)
                    {
                        st.store.active_profile_name = matched_name.clone();
                        st.bindings_dirty = true;
                        st.status_message = format!("Auto-Switch ke profil: {}", matched_name);
                    }
                }
            }
        }
    });

    // Thread 2: Polling Loop Kontroler 500Hz
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

        let (initial_gyro_cfg, initial_bindings) = {
            let st = shared.lock().unwrap();
            let prof = st.store.active_profile();
            (prof.gyro_cfg, prof.bindings.clone())
        };

        let mut gyro_processor = GyroProcessor::new(initial_gyro_cfg);

        let default_set = ActionSet {
            name: "ActiveSet".to_string(),
            bindings: initial_bindings,
            layers: HashMap::new(),
        };
        let mut action_engine = ActionEngine::new(default_set);
        let mut virtual_out = create_platform_output();

        let mut active_gamepad: Option<sdl3::gamepad::Gamepad> = None;
        let mut pressed_buttons = HashSet::new();
        let mut raw_left_stick = Vec2::ZERO;
        let mut raw_right_stick = Vec2::ZERO;
        let mut latest_gyro = Vec3::ZERO;

        let mut last_tick = Instant::now();
        let target_frame_time = Duration::from_millis(2);

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

            let (stick_cfg, gyro_cfg, need_recal, updated_bindings) = {
                let mut st = shared.lock().unwrap();
                let recal = st.recalibrate_gyro;
                st.recalibrate_gyro = false;

                let dirty = st.bindings_dirty;
                st.bindings_dirty = false;

                let prof = st.store.active_profile();
                let new_binds = if dirty {
                    Some(prof.bindings.clone())
                } else {
                    None
                };

                (prof.stick_cfg, prof.gyro_cfg, recal, new_binds)
            };

            if need_recal {
                gyro_processor.calibrator.reset();
            }
            gyro_processor.config = gyro_cfg;

            if let Some(binds) = updated_bindings {
                if let Some(set) = action_engine.sets.get_mut(&action_engine.active_set) {
                    set.bindings = binds;
                }
            }

            if active_gamepad.is_some() {
                let processed_l = process_stick(raw_left_stick.x, raw_left_stick.y, &stick_cfg);
                let processed_r = process_stick(raw_right_stick.x, raw_right_stick.y, &stick_cfg);
                let gyro_delta = gyro_processor.process(latest_gyro, dt);
                let triggered_actions = action_engine
                    .update(&pressed_buttons, now)
                    .into_iter()
                    .collect();

                virtual_out.send(&OutputState {
                    left_stick: processed_l,
                    right_stick: processed_r,
                    gyro_mouse_delta: gyro_delta,
                    actions: triggered_actions,
                });

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
