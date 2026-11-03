use std::f32::consts::{PI, TAU};

use glam::{IVec3, Vec3};
use vx_core::CHUNK_SIZE;
use vx_render::FlyCamera;
use vx_world::{BlockBox, TerrainGenerator, TerrainSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Small,
    Default,
    Far,
    Cave,
    Mountain,
    FlyFast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CameraPath {
    Orbit,
    Tunnel,
    GroundWalk,
    FlyWithTeleport,
}

const TUNNEL: BlockBox = BlockBox {
    min: IVec3::new(-240, 8, -3),
    max: IVec3::new(240, 16, 3),
};
const TUNNEL_TRAVEL: f32 = 200.0;
const TUNNEL_SWAY_RADIANS: f32 = 0.35;

const ORBIT_HEIGHT: f32 = 150.0;
const LOOK_AHEAD_RADIANS: f32 = 0.6;
const LOOK_DOWN_HEIGHT: f32 = 70.0;

const EYE_ABOVE_SURFACE: f32 = 2.7;
const WALK_PAN_RADIANS: f32 = 0.6;
const WALK_PITCH: f32 = -0.05;

const FLY_ALTITUDE: f32 = 125.0;
const FLY_PITCH: f32 = -0.25;

impl Scenario {
    pub const NAMES: &str = "small|default|far|cave|mountain|fly-fast";

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "small" => Some(Self::Small),
            "default" => Some(Self::Default),
            "far" => Some(Self::Far),
            "cave" => Some(Self::Cave),
            "mountain" => Some(Self::Mountain),
            "fly-fast" => Some(Self::FlyFast),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Default => "default",
            Self::Far => "far",
            Self::Cave => "cave",
            Self::Mountain => "mountain",
            Self::FlyFast => "fly-fast",
        }
    }

    pub fn view_radius(self) -> u32 {
        match self {
            Self::Small => 8,
            Self::Default | Self::Cave | Self::Mountain | Self::FlyFast => 16,
            Self::Far => 32,
        }
    }

    pub fn terrain_settings(self) -> TerrainSettings {
        TerrainSettings {
            carved_air: (self == Self::Cave).then_some(TUNNEL),
            ..TerrainSettings::default()
        }
    }

    fn camera_path(self) -> CameraPath {
        match self {
            Self::Small | Self::Default | Self::Far => CameraPath::Orbit,
            Self::Cave => CameraPath::Tunnel,
            Self::Mountain => CameraPath::GroundWalk,
            Self::FlyFast => CameraPath::FlyWithTeleport,
        }
    }

    /// Camera at `progress` in `[0, 1)` along the scenario's fixed path.
    pub fn camera_at(self, terrain: &TerrainGenerator, progress: f32) -> FlyCamera {
        let half_extent = self.view_radius() as f32 * CHUNK_SIZE as f32;
        match self.camera_path() {
            CameraPath::Orbit => orbit_camera(half_extent * 0.5, progress),
            CameraPath::Tunnel => tunnel_camera(progress),
            CameraPath::GroundWalk => ground_walk_camera(terrain, half_extent, progress),
            CameraPath::FlyWithTeleport => fly_camera(half_extent, progress),
        }
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn orbit_camera(orbit_radius: f32, progress: f32) -> FlyCamera {
    let angle = progress * TAU;
    let point_on_orbit =
        |angle: f32| Vec3::new(angle.cos() * orbit_radius, 0.0, angle.sin() * orbit_radius);
    let position = point_on_orbit(angle) + Vec3::Y * ORBIT_HEIGHT;
    let target = point_on_orbit(angle + LOOK_AHEAD_RADIANS) + Vec3::Y * LOOK_DOWN_HEIGHT;
    FlyCamera::looking_at(position, target)
}

fn tunnel_camera(progress: f32) -> FlyCamera {
    let centre = (TUNNEL.min + TUNNEL.max).as_vec3() * 0.5;
    let position = Vec3::new(
        lerp(-TUNNEL_TRAVEL, TUNNEL_TRAVEL, progress),
        centre.y,
        centre.z,
    );
    let mut camera = FlyCamera::looking_at(position, position + Vec3::X);
    camera.yaw = (progress * 3.0 * TAU).sin() * TUNNEL_SWAY_RADIANS;
    camera
}

fn ground_walk_camera(terrain: &TerrainGenerator, half_extent: f32, progress: f32) -> FlyCamera {
    let x = lerp(-0.6 * half_extent, 0.6 * half_extent, progress);
    let z = 0.15 * half_extent;
    let surface = terrain.surface_height_at(x.floor() as i32, z.floor() as i32);
    let mut camera = FlyCamera::looking_at(
        Vec3::new(x, surface as f32 + EYE_ABOVE_SURFACE, z),
        Vec3::new(x + 1.0, 0.0, z),
    );
    camera.yaw = (progress * TAU).sin() * WALK_PAN_RADIANS;
    camera.pitch = WALK_PITCH;
    camera
}

fn fly_camera(half_extent: f32, progress: f32) -> FlyCamera {
    let leg_progress = (progress * 2.0).fract();
    let along = lerp(-0.75 * half_extent, 0.75 * half_extent, leg_progress);
    let offset = 0.3 * half_extent;
    let (position, yaw) = if progress < 0.5 {
        (Vec3::new(along, FLY_ALTITUDE, -offset), 0.0)
    } else {
        (Vec3::new(offset, FLY_ALTITUDE, along), PI * 0.5)
    };
    let mut camera = FlyCamera::looking_at(position, position + Vec3::X);
    camera.yaw = yaw;
    camera.pitch = FLY_PITCH;
    camera
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_names_round_trip() {
        for name in Scenario::NAMES.split('|') {
            let scenario = Scenario::from_name(name).expect("listed name parses");
            assert_eq!(scenario.name(), name);
        }
    }

    #[test]
    fn tunnel_stays_sealed_below_the_surface() {
        let terrain = TerrainGenerator::new(TerrainSettings::default());
        let lowest_surface = (TUNNEL.min.x..TUNNEL.max.x)
            .flat_map(|x| (TUNNEL.min.z - 1..=TUNNEL.max.z).map(move |z| (x, z)))
            .map(|(x, z)| terrain.surface_height_at(x, z))
            .min()
            .expect("tunnel footprint is not empty");
        assert!(
            lowest_surface > TUNNEL.max.y + 4,
            "surface dips to {lowest_surface}, tunnel roof at {}",
            TUNNEL.max.y
        );
    }

    #[test]
    fn tunnel_camera_stays_inside_the_tunnel() {
        for step in 0..=100 {
            let position = tunnel_camera(step as f32 / 100.0).position;
            assert!(position.cmpge(TUNNEL.min.as_vec3()).all());
            assert!(position.cmplt(TUNNEL.max.as_vec3()).all());
        }
    }

    #[test]
    fn orbit_path_matches_the_original_benchmark() {
        let camera =
            Scenario::Default.camera_at(&TerrainGenerator::new(TerrainSettings::default()), 0.25);
        let expected = orbit_camera(16.0 * 32.0 * 0.5, 0.25);
        assert_eq!(camera.position, expected.position);
        assert!((camera.position - Vec3::new(0.0, ORBIT_HEIGHT, 256.0)).length() < 1e-3);
    }
}
