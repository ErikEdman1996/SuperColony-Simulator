use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    pub expansion_seconds: f32,
    pub army_fraction: f32,
    pub starting_population: u32,
    pub starting_stored_food: f64,
    pub zone_food_min: f64,
    pub zone_food_max: f64,
    pub harvest_per_ant_per_second: f64,
    pub harvest_limit_per_zone_per_second: f64,
    pub upkeep_per_ant_per_second: f64,
    pub births_per_second: f64,
    pub food_per_birth: f64,
    pub birth_reserve_seconds: f64,
    pub starvation_seconds: f64,
    pub ant_speed: f32,
    pub ant_health: f32,
    pub attack_damage: f32,
    pub attack_interval: f32,
}

impl Tuning {
    pub fn load() -> Result<Self, String> {
        // Resolve from the working directory first, then the source tree.
        let path = if std::path::Path::new("tuning.toml").exists() {
            std::path::PathBuf::from("tuning.toml")
        } else {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tuning.toml")
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        let tuning: Self = toml::from_str(&text).map_err(|e| e.to_string())?;
        tuning.validate()?;
        Ok(tuning)
    }

    pub fn validate(&self) -> Result<(), String> {
        let positive = [
            self.expansion_seconds as f64,
            self.starting_stored_food,
            self.zone_food_min,
            self.zone_food_max,
            self.harvest_per_ant_per_second,
            self.harvest_limit_per_zone_per_second,
            self.upkeep_per_ant_per_second,
            self.births_per_second,
            self.food_per_birth,
            self.birth_reserve_seconds,
            self.starvation_seconds,
            self.ant_speed as f64,
            self.ant_health as f64,
            self.attack_damage as f64,
            self.attack_interval as f64,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err("Tuning rates and quantities must be finite and positive".into());
        }
        if !self.army_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.army_fraction)
            || self.army_fraction == 0.0
            || self.starting_population < 2
            || self.zone_food_max < self.zone_food_min
        {
            return Err("Invalid army fraction, starting population, or food range".into());
        }
        Ok(())
    }
}
