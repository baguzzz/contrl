use glam::Vec2;
use mapper_core::actions::GameAction;
use std::collections::HashSet;

pub struct OutputState {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub gyro_mouse_delta: Vec2,
    pub actions: HashSet<GameAction>,
}

pub trait VirtualOutput {
    fn send(&mut self, state: &OutputState);
}

#[cfg(target_os = "windows")]
pub struct WindowsOutput;

#[cfg(target_os = "windows")]
impl VirtualOutput for WindowsOutput {
    fn send(&mut self, state: &OutputState) {
        let dx = state.gyro_mouse_delta.x.round() as i32;
        let dy = state.gyro_mouse_delta.y.round() as i32;

        if dx != 0 || dy != 0 {
            unsafe {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                    mouse_event,
                    MOUSEEVENTF_MOVE,
                };

                mouse_event(MOUSEEVENTF_MOVE, dx, dy, 0, 0);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub struct DummyOutput;

#[cfg(not(target_os = "windows"))]
impl VirtualOutput for DummyOutput {
    fn send(&mut self, state: &OutputState) {
        if !state.actions.is_empty()
            || state.gyro_mouse_delta.length() > 0.5
        {
            println!(
                "[Virtual Out] Stick L: {:?} | Gyro Mouse: {:?} | Actions: {:?}",
                state.left_stick,
                state.gyro_mouse_delta,
                state.actions
            );
        }
    }
}

pub fn create_platform_output() -> Box<dyn VirtualOutput> {
    #[cfg(target_os = "windows")]
    {
        println!("[Windows] Output mouse gyro aktif");
        Box::new(WindowsOutput)
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("[Linux/Dev] Menggunakan dummy virtual output");
        Box::new(DummyOutput)
    }
}