use bytemuck::{Pod, Zeroable};
use glam::camera::rh::{proj::directx, view};
use glam::{IVec3, Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    /// Projection times view, in camera-relative space: world positions enter as `block - anchor`.
    pub view_proj: Mat4,
    pub anchor: [i32; 4],
    /// Eye position relative to `anchor`, each component in `[0, 1)`.
    pub eye: [f32; 4],
}

impl CameraUniform {
    pub fn anchor(&self) -> IVec3 {
        IVec3::from_slice(&self.anchor[..3])
    }

    pub fn eye(&self) -> Vec3 {
        Vec3::from_slice(&self.eye[..3])
    }

    /// `block` in the camera-relative space of `view_proj`; exact while the difference fits f32.
    pub fn relative(&self, block: IVec3) -> Vec3 {
        (block - self.anchor()).as_vec3()
    }
}

/// `anchor + position` is the eye in world blocks; `rebase` keeps `position` small so f32
/// precision does not depend on distance from the world origin (ADR-0016).
#[derive(Clone, Copy, Debug)]
pub struct FlyCamera {
    pub anchor: IVec3,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub vertical_fov: f32,
    pub near: f32,
}

impl FlyCamera {
    pub fn looking_at(position: Vec3, target: Vec3) -> Self {
        let forward = (target - position).normalize();
        let mut camera = Self {
            anchor: IVec3::ZERO,
            position,
            yaw: forward.z.atan2(forward.x),
            pitch: forward.y.asin(),
            vertical_fov: 70f32.to_radians(),
            near: 0.1,
        };
        camera.rebase();
        camera
    }

    pub fn rebase(&mut self) {
        let whole = self.position.floor();
        self.anchor += whole.as_ivec3();
        self.position -= whole;
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }

    pub fn flat_right(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, self.yaw.cos())
    }

    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn uniform(&self, aspect: f32) -> CameraUniform {
        let mut rebased = *self;
        rebased.rebase();
        let view = view::look_to_mat4(rebased.position, rebased.forward(), Vec3::Y);
        let projection =
            directx::perspective_infinite_reverse(rebased.vertical_fov, aspect, rebased.near);
        CameraUniform {
            view_proj: projection * view,
            anchor: rebased.anchor.extend(0).to_array(),
            eye: rebased.position.extend(1.0).to_array(),
        }
    }
}
