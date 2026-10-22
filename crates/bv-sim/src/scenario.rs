use std::{env, path::Path};

use anyhow::{Context, Result, bail};
use bv_renderer::{DroneCameraConfig, WorldChoice};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub format_version: u32,
    pub name: String,
    pub home: Home,
    pub camera: Camera,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub width: u32,
    pub height: u32,
    pub horizontal_fov_deg: f32,
    pub jpeg_quality: u8,
}

impl Scenario {
    pub fn load(directory: &Path, world: &WorldChoice) -> Result<Self> {
        let path = directory.join(format!("{}.toml", world.name()));
        let source = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "cannot read scenario {}. Install the complete release or set --scenarios-dir",
                path.display()
            )
        })?;
        let scenario: Self = toml::from_str(&source)
            .with_context(|| format!("invalid scenario {}", path.display()))?;
        scenario.validate()?;
        if scenario.name != world.name() {
            bail!(
                "scenario name {} does not match selected world {}",
                scenario.name,
                world.name()
            );
        }
        let home = world.px4_home();
        if (home.latitude - scenario.home.latitude).abs() > 1e-9
            || (home.longitude - scenario.home.longitude).abs() > 1e-9
        {
            bail!(
                "{} has a fixed visual origin; scenario home coordinates must match the renderer ({}, {})",
                world.name(),
                home.latitude,
                home.longitude
            );
        }
        Ok(scenario)
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            bail!("unsupported scenario format {}", self.format_version);
        }
        if self.name.is_empty() {
            bail!("scenario name must not be empty");
        }
        let latitude_limit = 89.0;
        if !self.home.latitude.is_finite()
            || self.home.latitude.abs() > latitude_limit
            || !self.home.longitude.is_finite()
            || self.home.longitude.abs() > 180.0
            || !self.home.altitude_m.is_finite()
        {
            bail!(
                "scenario requires finite home latitude within ±{latitude_limit}°, longitude within ±180°, and altitude in meters"
            );
        }
        self.camera.validate()
    }
}

impl Camera {
    fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.width > 8192 || self.height > 8192 {
            bail!("camera dimensions must be between 1 and 8192 pixels");
        }
        if !self.horizontal_fov_deg.is_finite()
            || self.horizontal_fov_deg <= 0.0
            || self.horizontal_fov_deg >= 180.0
        {
            bail!("camera horizontal field of view must be between 0 and 180 degrees");
        }
        if !(1..=100).contains(&self.jpeg_quality) {
            bail!("JPEG quality must be 1 through 100");
        }
        Ok(())
    }

    pub fn renderer_config(&self) -> Result<(DroneCameraConfig, u8)> {
        let camera = Self {
            width: env_number("BV_CAMERA_WIDTH", self.width)?,
            height: env_number("BV_CAMERA_HEIGHT", self.height)?,
            horizontal_fov_deg: env_number("BV_CAMERA_HFOV_DEG", self.horizontal_fov_deg)?,
            jpeg_quality: env_number("BV_CAMERA_JPEG_QUALITY", self.jpeg_quality)?,
        };
        camera.validate()?;
        Ok((
            DroneCameraConfig {
                width: camera.width,
                height: camera.height,
                horizontal_fov: camera.horizontal_fov_deg.to_radians(),
                ..DroneCameraConfig::default()
            },
            camera.jpeg_quality,
        ))
    }
}

fn env_number<T: std::str::FromStr>(name: &str, fallback: T) -> Result<T>
where
    T::Err: std::fmt::Display + Send + Sync + 'static,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid {name}: {error}")),
        Err(env::VarError::NotPresent) => Ok(fallback),
        Err(error) => Err(error).with_context(|| format!("invalid {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_suas_scenario_is_valid() {
        let scenario: Scenario =
            toml::from_str(include_str!("../../../scenarios/suas_2026.toml")).unwrap();
        scenario.validate().unwrap();
    }

    #[test]
    fn bad_origins_and_camera_settings_are_rejected() {
        let mut scenario: Scenario =
            toml::from_str(include_str!("../../../scenarios/suas_2026.toml")).unwrap();
        scenario.home.latitude = f64::NAN;
        assert!(scenario.validate().is_err());
        scenario.home.latitude = 36.2;
        scenario.camera.horizontal_fov_deg = 180.0;
        assert!(scenario.validate().is_err());
        scenario.camera.horizontal_fov_deg = 80.0;
        scenario.camera.jpeg_quality = 0;
        assert!(scenario.validate().is_err());
    }

    #[test]
    fn suas_coordinates_match_the_visual_origin() {
        let world = WorldChoice::Suas2026;
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
        let scenario = Scenario::load(&directory, &world).unwrap();
        assert_eq!(scenario.home.latitude, world.px4_home().latitude);
        assert_eq!(scenario.home.longitude, world.px4_home().longitude);
    }

    #[test]
    fn rejects_home_that_disagrees_with_suas_visuals() {
        let directory = env::temp_dir().join(format!("bv-sim-scenario-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source = include_str!("../../../scenarios/suas_2026.toml").replace("36.214675", "36.2");
        std::fs::write(directory.join("suas_2026.toml"), source).unwrap();
        let result = Scenario::load(&directory, &WorldChoice::Suas2026);
        std::fs::remove_dir_all(directory).unwrap();
        assert!(result.is_err());
    }
}
