//! Camara.
//!
//! Hay cuatro modos y todos se mueven en las tres dimensiones. Los tres
//! primeros estan atados a la nave; el cuarto se despega para poder
//! recorrer el sistema por fuera.

use crate::math::{Mat4, Vec3};
use crate::scene::ship::Ship;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// Tercera persona detras de la nave.
    Persecucion,
    /// Desde la cabina.
    Cabina,
    /// Girando alrededor del planeta seleccionado.
    Orbital,
    /// Vuelo libre, independiente de la nave.
    Libre,
}

impl CameraMode {
    pub fn nombre(&self) -> &'static str {
        match self {
            CameraMode::Persecucion => "PERSECUCION",
            CameraMode::Cabina => "CABINA",
            CameraMode::Orbital => "ORBITAL",
            CameraMode::Libre => "LIBRE",
        }
    }

    pub fn siguiente(self) -> Self {
        match self {
            CameraMode::Persecucion => CameraMode::Cabina,
            CameraMode::Cabina => CameraMode::Orbital,
            CameraMode::Orbital => CameraMode::Libre,
            CameraMode::Libre => CameraMode::Persecucion,
        }
    }
}

pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    pub mode: CameraMode,

    /// Distancia y altura de la camara de persecucion.
    pub chase_distance: f32,
    pub chase_height: f32,

    /// Estado del modo orbital.
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32,

    /// Estado del modo libre.
    pub free_yaw: f32,
    pub free_pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 8.0, 60.0),
            target: Vec3::ZERO,
            up: Vec3::UP,
            fov_y: crate::math::to_radians(62.0),
            near: 0.05,
            far: 4000.0,
            mode: CameraMode::Persecucion,
            chase_distance: 4.2,
            chase_height: 1.25,
            orbit_yaw: 0.0,
            orbit_pitch: 0.25,
            orbit_distance: 8.0,
            free_yaw: 0.0,
            free_pitch: 0.0,
        }
    }
}

#[inline]
fn suavizado(k: f32, dt: f32) -> f32 {
    1.0 - (-k * dt).exp()
}

impl Camera {
    pub fn view(&self) -> Mat4 {
        Mat4::look_at(self.position, self.target, self.up)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fov_y, aspect, self.near, self.far)
    }

    /// Base ortonormal de la camara: derecha, arriba y adelante.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = (self.target - self.position).normalized();
        let mut right = forward.cross(self.up).normalized();
        if right == Vec3::ZERO {
            right = Vec3::new(1.0, 0.0, 0.0);
        }
        let up = right.cross(forward).normalized();
        (right, up, forward)
    }

    /// Camara de persecucion: queda detras y arriba de la nave y la
    /// alcanza de forma suave, de modo que al acelerar se abre un poco y
    /// al frenar se cierra.
    pub fn follow(&mut self, ship: &Ship, dt: f32) {
        let deseada = ship.position - ship.forward * self.chase_distance + ship.up * self.chase_height;
        let k = suavizado(7.0, dt);
        self.position = self.position.lerp(deseada, k);
        self.target = self.target.lerp(ship.position + ship.forward * 6.0, suavizado(10.0, dt));
        self.up = self.up.lerp(ship.up, suavizado(6.0, dt)).normalized();
    }

    /// Camara de cabina: pegada a la nave, sin suavizado.
    pub fn cockpit(&mut self, ship: &Ship) {
        self.position = ship.position + ship.up * 0.42 + ship.forward * 0.55;
        self.target = self.position + ship.forward;
        self.up = ship.up;
    }

    /// Camara orbital alrededor de un punto: gira sobre el objetivo sin
    /// cambiar de escena.
    pub fn orbit(&mut self, centro: Vec3, radio_objetivo: f32, dt: f32) {
        let distancia = self.orbit_distance.max(radio_objetivo * 1.6);
        let cp = self.orbit_pitch.cos();
        let offset = Vec3::new(
            self.orbit_yaw.cos() * cp,
            self.orbit_pitch.sin(),
            self.orbit_yaw.sin() * cp,
        ) * distancia;
        let k = suavizado(9.0, dt);
        self.position = self.position.lerp(centro + offset, k);
        self.target = self.target.lerp(centro, k);
        self.up = self.up.lerp(Vec3::UP, suavizado(4.0, dt)).normalized();
    }

    /// Camara libre: avanza en la direccion a la que mira, con los tres
    /// ejes disponibles.
    pub fn free_move(&mut self, avance: f32, lateral: f32, vertical: f32, velocidad: f32, dt: f32) {
        let cp = self.free_pitch.cos();
        let forward = Vec3::new(
            self.free_yaw.cos() * cp,
            self.free_pitch.sin(),
            self.free_yaw.sin() * cp,
        )
        .normalized();
        let right = forward.cross(Vec3::UP).normalized();
        let up = right.cross(forward).normalized();

        self.position += (forward * avance + right * lateral + up * vertical) * (velocidad * dt);
        self.target = self.position + forward;
        self.up = Vec3::UP;
    }
}
