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

// =========================================================
// IMPLEMENTASI WINDOWS: ViGEm (Xbox 360 Virtual) + Mouse API
// =========================================================
#[cfg(target_os = "windows")]
pub struct WindowsOutput {
    _client: vigem_client::Client,
    target: vigem_client::Xbox360Wired<vigem_client::Client>,
}

#[cfg(target_os = "windows")]
impl WindowsOutput {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = vigem_client::Client::connect()?;
        let id = vigem_client::TargetId::XBOX360_WIRED;
        let mut target = vigem_client::Xbox360Wired::new(client, id);
        target.plugin()?;
        target.wait_ready()?;
        println!("[Windows] Kontroler Virtual Xbox 360 aktif!");

        // Buat koneksi client kedua untuk disimpan jika perlu
        let client_holder = vigem_client::Client::connect()?;
        Ok(Self {
            _client: client_holder,
            target,
        })
    }
}

#[cfg(target_os = "windows")]
impl VirtualOutput for WindowsOutput {
    fn send(&mut self, state: &OutputState) {
        use vigem_client::{XButtons, XGamepad};

        let mut buttons = XButtons::default();
        if state.actions.contains(&GameAction::Jump) {
            buttons.raw |= XButtons::A;
        }
        if state.actions.contains(&GameAction::Reload) {
            buttons.raw |= XButtons::X;
        }
        if state.actions.contains(&GameAction::Interact) {
            buttons.raw |= XButtons::Y;
        }

        // Konversi float [-1.0 .. 1.0] ke i16 [-32768 .. 32767]
        let lx = (state.left_stick.x.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ly = (state.left_stick.y.clamp(-1.0, 1.0) * 32767.0) as i16;
        let rx = (state.right_stick.x.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ry = (state.right_stick.y.clamp(-1.0, 1.0) * 32767.0) as i16;

        let gamepad = XGamepad {
            buttons,
            thumb_lx: lx,
            thumb_ly: ly,
            thumb_rx: rx,
            thumb_ry: ry,
            ..Default::default()
        };

        let _ = self.target.update(&gamepad);

        // Kirim pergerakan Gyro sebagai gerakan Mouse relatif di Windows
        let dx = state.gyro_mouse_delta.x.round() as i32;
        let dy = state.gyro_mouse_delta.y.round() as i32;
        if dx != 0 || dy != 0 {
            unsafe {
                use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                    mouse_event, MOUSEEVENTF_MOVE,
                };
                mouse_event(MOUSEEVENTF_MOVE, dx, dy, 0, 0);
            }
        }
    }
}

// =========================================================
// IMPLEMENTASI LINUX / DEV (Console Logging)
// =========================================================
#[cfg(not(target_os = "windows"))]
pub struct DummyOutput;

#[cfg(not(target_os = "windows"))]
impl VirtualOutput for DummyOutput {
    fn send(&mut self, state: &OutputState) {
        if !state.actions.is_empty() || state.gyro_mouse_delta.length() > 0.5 {
            println!(
                "[Virtual Out] Stick L: {:?} | Gyro Mouse: {:?} | Actions: {:?}",
                state.left_stick, state.gyro_mouse_delta, state.actions
            );
        }
    }
}

pub fn create_platform_output() -> Box<dyn VirtualOutput> {
    #[cfg(target_os = "windows")]
    {
        Box::new(
            WindowsOutput::new()
                .expect("Gagal terhubung ke ViGEmBus. Pastikan driver ViGEmBus sudah terinstal!"),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        println!("[Linux/Dev] Menggunakan Dummy Virtual Output");
        Box::new(DummyOutput)
    }
}
