use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Representasi tombol fisik standar pada kontroler
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicalButton {
    South,      // A (Xbox) / Cross (PS)
    East,       // B (Xbox) / Circle (PS)
    West,       // X (Xbox) / Square (PS)
    North,      // Y (Xbox) / Triangle (PS)
    LeftBumper, // LB / L1
    RightBumper,// RB / R1
    LeftTriggerBtn,
    RightTriggerBtn,
    BackLeft,   // Tombol Paddle belakang (Steam Deck / Elite)
    BackRight,
}

/// Jenis aktivasi tombol ala Steam Input
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActivatorType {
    /// Langsung aktif saat tombol ditekan
    RegularPress,
    /// Aktif setelah tombol ditahan selama durasi tertentu (ms)
    LongPress { hold_duration_ms: u64 },
    /// Aktif jika tombol ditekan dua kali dalam jendela waktu (ms)
    DoubleTap { window_ms: u64 },
    /// Aktif hanya jika tombol modifier (misal LB) sedang ditahan bersamaan
    Chord { modifier: PhysicalButton },
}

/// Aksi abstrak di dalam game atau sistem
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameAction {
    Jump,
    Reload,
    Interact,
    QuickSave,
    ToggleGyro,
    ApplyLayer(String),  // Mengaktifkan Action Layer sementara
    RemoveLayer(String), // Menghapus Action Layer
    SwitchSet(String),   // Pindah Action Set permanen
}

/// Satu aturan pemetaan dari pemicu fisik ke aksi game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionBinding {
    pub trigger: PhysicalButton,
    pub activator: ActivatorType,
    pub action: GameAction,
}

/// Action Layer: Kumpulan binding yang menimpa Action Set utama saat aktif
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionLayer {
    pub name: String,
    pub bindings: Vec<ActionBinding>,
}

/// Action Set: Profil utama (misal: "In-Game", "Menu", "Vehicle")
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionSet {
    pub name: String,
    pub bindings: Vec<ActionBinding>,
    pub layers: HashMap<String, ActionLayer>,
}

/// Pelacak status waktu tekan tiap tombol fisik (State Machine)
#[derive(Debug, Default)]
struct ButtonTracker {
    is_pressed: bool,
    pressed_at: Option<Instant>,
    last_tap_at: Option<Instant>,
    long_press_triggered: bool,
    double_tap_triggered: bool,
}

/// Mesin utama pengelola Action Sets, Layers, dan evaluasi input
pub struct ActionEngine {
    pub sets: HashMap<String, ActionSet>,
    pub active_set: String,
    pub active_layers: Vec<String>, // Tumpukan (Stack) layer aktif; paling belakang = prioritas tertinggi
    trackers: HashMap<PhysicalButton, ButtonTracker>,
}

impl ActionEngine {
    pub fn new(default_set: ActionSet) -> Self {
        let name = default_set.name.clone();
        let mut sets = HashMap::new();
        sets.insert(name.clone(), default_set);

        Self {
            sets,
            active_set: name,
            active_layers: Vec::new(),
            trackers: HashMap::new(),
        }
    }

    pub fn push_layer(&mut self, layer_name: &str) {
        if !self.active_layers.contains(&layer_name.to_string()) {
            self.active_layers.push(layer_name.to_string());
        }
    }

    pub fn pop_layer(&mut self, layer_name: &str) {
        self.active_layers.retain(|l| l != layer_name);
    }

    /// Evaluasi kondisi tombol fisik pada frame saat ini dan kembalikan daftar GameAction yang aktif
    pub fn update(
        &mut self,
        pressed_buttons: &HashSet<PhysicalButton>,
        now: Instant,
    ) -> Vec<GameAction> {
        let mut triggered_actions = Vec::new();

        // 1. Perbarui status waktu (tracker) untuk semua tombol yang sedang/pernah ditekan
        let all_known_buttons: Vec<PhysicalButton> = pressed_buttons
            .iter()
            .copied()
            .chain(self.trackers.keys().copied())
            .collect();

        for btn in all_known_buttons {
            let is_down = pressed_buttons.contains(&btn);
            let tracker = self.trackers.entry(btn).or_default();

            if is_down && !tracker.is_pressed {
                // Transisi: Baru saja ditekan (Just Pressed)
                tracker.is_pressed = true;
                tracker.pressed_at = Some(now);
                tracker.long_press_triggered = false;

                // Cek apakah masuk jendela Double Tap dari tekanan sebelumnya
                if let Some(last_tap) = tracker.last_tap_at {
                    tracker.double_tap_triggered = true;
                    tracker.last_tap_at = None; // Reset agar triple-tap tidak memicu ganda
                    let _ = last_tap; // Digunakan saat evaluasi durasi window di bawah
                } else {
                    tracker.double_tap_triggered = false;
                    tracker.last_tap_at = Some(now);
                }
            } else if !is_down && tracker.is_pressed {
                // Transisi: Baru saja dilepas (Just Released)
                tracker.is_pressed = false;
                tracker.pressed_at = None;
                tracker.long_press_triggered = false;
                tracker.double_tap_triggered = false;
            }
        }

        // 2. Kumpulkan binding aktif: Layer teratas menimpa (override) Layer di bawahnya & Action Set dasar
        let current_set = match self.sets.get(&self.active_set) {
            Some(s) => s.clone(),
            None => return triggered_actions,
        };

        let mut effective_bindings: HashMap<(PhysicalButton, String), ActionBinding> = HashMap::new();

        // Masukkan binding dari Action Set dasar
        for b in &current_set.bindings {
            let key = (b.trigger, format!("{:?}", b.activator));
            effective_bindings.insert(key, b.clone());
        }

        // Timpa dengan binding dari Action Layers yang sedang aktif secara berurutan
        for layer_name in &self.active_layers {
            if let Some(layer) = current_set.layers.get(layer_name) {
                for b in &layer.bindings {
                    let key = (b.trigger, format!("{:?}", b.activator));
                    effective_bindings.insert(key, b.clone());
                }
            }
        }

        // 3. Evaluasi setiap binding terhadap state tracker saat ini
        let mut internal_commands = Vec::new();

        for binding in effective_bindings.values() {
            let Some(tracker) = self.trackers.get_mut(&binding.trigger) else {
                continue;
            };

            let is_active = match &binding.activator {
                ActivatorType::RegularPress => tracker.is_pressed,

                ActivatorType::LongPress { hold_duration_ms } => {
                    if tracker.is_pressed {
                        if let Some(start) = tracker.pressed_at {
                            let elapsed = now.duration_since(start);
                            if elapsed >= Duration::from_millis(*hold_duration_ms) {
                                tracker.long_press_triggered = true;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }

                ActivatorType::DoubleTap { window_ms } => {
                    if tracker.is_pressed && tracker.double_tap_triggered {
                        if let Some(start) = tracker.pressed_at {
                            // Validasi jarak waktu masih dalam batas window_ms
                            now.duration_since(start) <= Duration::from_millis(*window_ms)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }

                ActivatorType::Chord { modifier } => {
                    tracker.is_pressed && pressed_buttons.contains(modifier)
                }
            };

            if is_active {
                match &binding.action {
                    GameAction::ApplyLayer(name) => internal_commands.push(GameAction::ApplyLayer(name.clone())),
                    GameAction::RemoveLayer(name) => internal_commands.push(GameAction::RemoveLayer(name.clone())),
                    GameAction::SwitchSet(name) => internal_commands.push(GameAction::SwitchSet(name.clone())),
                    other => triggered_actions.push(other.clone()),
                }
            }
        }

        // 4. Jalankan perintah internal perpindahan Layer / Action Set
        for cmd in internal_commands {
            match cmd {
                GameAction::ApplyLayer(name) => self.push_layer(&name),
                GameAction::RemoveLayer(name) => self.pop_layer(&name),
                GameAction::SwitchSet(name) => {
                    if self.sets.contains_key(&name) {
                        self.active_set = name;
                        self.active_layers.clear();
                    }
                }
                _ => {}
            }
        }

        triggered_actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_long_press_and_chord_activators() {
        let mut gameplay_set = ActionSet {
            name: "Gameplay".to_string(),
            bindings: vec![
                // Tekan lama tombol West (X) selama 500ms untuk Reload
                ActionBinding {
                    trigger: PhysicalButton::West,
                    activator: ActivatorType::LongPress { hold_duration_ms: 500 },
                    action: GameAction::Reload,
                },// Kombinasi LB + South (A) untuk QuickSave
ActionBinding {
trigger: PhysicalButton::South,
activator: ActivatorType::Chord { modifier: PhysicalButton::LeftBumper },
action: GameAction::QuickSave,
},
],
layers: HashMap::new(),
};
// Tambahkan Action Layer khusus saat membidik (Aiming)
gameplay_set.layers.insert(
"AimLayer".to_string(),
ActionLayer {
name: "AimLayer".to_string(),
bindings: vec![ActionBinding {
trigger: PhysicalButton::South,
activator: ActivatorType::RegularPress,
action: GameAction::ToggleGyro,
}],
},
);
let mut engine = ActionEngine::new(gameplay_set);
let t0 = Instant::now();
let mut pressed = HashSet::new();
// Frame 1: Baru menekan West (0ms) -> Belum memicu Reload
pressed.insert(PhysicalButton::West);
let actions = engine.update(&pressed, t0);
assert!(!actions.contains(&GameAction::Reload));
// Frame 2: Ditahan selama 600ms -> Memicu Reload
let actions_after_hold = engine.update(&pressed, t0 + Duration::from_millis(600));
assert!(actions_after_hold.contains(&GameAction::Reload));
// Frame 3: Uji Chord (LB + South)
pressed.clear();
pressed.insert(PhysicalButton::LeftBumper);
pressed.insert(PhysicalButton::South);
let chord_actions = engine.update(&pressed, t0 + Duration::from_millis(700));
assert!(chord_actions.contains(&GameAction::QuickSave));
// Frame 4: Aktifkan AimLayer -> Tombol South sekarang memicu ToggleGyro
pressed.clear();
engine.push_layer("AimLayer");
pressed.insert(PhysicalButton::South);
let layer_actions = engine.update(&pressed, t0 + Duration::from_millis(800));
assert!(layer_actions.contains(&GameAction::ToggleGyro));
}
}
