use mapper_core::actions::{ActionBinding, ActivatorType, GameAction, PhysicalButton};
use mapper_core::gyro::GyroConfig;
use mapper_core::StickConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const PROFILE_FILE: &str = "profiles.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct GameProfile {
    pub name: String,
    /// Kata kunci nama proses/jendela (misal: "valorant", "cyberpunk", "notepad")
    /// Kosongkan untuk profil "Default"
    pub window_keyword: String,
    pub stick_cfg: StickConfig,
    pub gyro_cfg: GyroConfig,
    pub bindings: Vec<ActionBinding>,
}

impl GameProfile {
    pub fn default_profile() -> Self {
        Self {
            name: "Default".to_string(),
            window_keyword: "".to_string(),
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
            bindings: vec![
                ActionBinding {
                    trigger: PhysicalButton::South,
                    activator: ActivatorType::RegularPress,
                    action: GameAction::Jump,
                },
                ActionBinding {
                    trigger: PhysicalButton::West,
                    activator: ActivatorType::LongPress {
                        hold_duration_ms: 400,
                    },
                    action: GameAction::Reload,
                },
            ],
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    pub profiles: HashMap<String, GameProfile>,
    pub active_profile_name: String,
}

impl ProfileStore {
    pub fn load_or_default() -> Self {
        if Path::new(PROFILE_FILE).exists() {
            if let Ok(content) = fs::read_to_string(PROFILE_FILE) {
                if let Ok(store) = serde_json::from_str::<ProfileStore>(&content) {
                    if !store.profiles.is_empty() {
                        return store;
                    }
                }
            }
        }

        let mut profiles = HashMap::new();
        let def = GameProfile::default_profile();
        profiles.insert(def.name.clone(), def);

        let store = Self {
            profiles,
            active_profile_name: "Default".to_string(),
        };
        let _ = store.save();
        store
    }

    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(PROFILE_FILE, json).map_err(|e| e.to_string())
    }

    pub fn active_profile(&self) -> &GameProfile {
        self.profiles
            .get(&self.active_profile_name)
            .or_else(|| self.profiles.get("Default"))
            .unwrap()
    }

    pub fn active_profile_mut(&mut self) -> &mut GameProfile {
        if !self.profiles.contains_key(&self.active_profile_name) {
            self.active_profile_name = "Default".to_string();
        }
        self.profiles.get_mut(&self.active_profile_name).unwrap()
    }
}
