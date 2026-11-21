//! Algebra lineal minima escrita para este renderizador.

pub mod matrix;
pub mod vector;

pub use matrix::Mat4;
pub use vector::{Vec2, Vec3, Vec4};

pub const PI: f32 = std::f32::consts::PI;
pub const TAU: f32 = std::f32::consts::TAU;

pub fn to_radians(deg: f32) -> f32 {
    deg * PI / 180.0
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolacion suave de Hermite entre dos bordes, el `smoothstep` de GLSL.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() <= 1e-12 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Equivalente al `step` de GLSL.
pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}
