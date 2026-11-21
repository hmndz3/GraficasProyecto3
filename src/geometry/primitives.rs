//! Piezas basicas con las que se arman los modelos propios.
//!
//! Los modelos de la nave y de los objetos de superficie no vienen de
//! ningun archivo exportado: se componen aqui uniendo cajas, conos y
//! cilindros transformados.

use super::mesh::{Mesh, Vertex};
use crate::math::{Vec2, Vec3, TAU};

/// Caja centrada en el origen con caras planas (normales por cara, sin
/// suavizar, para que se lean las aristas).
pub fn box_mesh(half: Vec3) -> Mesh {
    let h = half;
    let corners = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];

    // Cada cara con sus cuatro esquinas en orden antihorario vista desde
    // afuera, y su normal.
    let faces: [([usize; 4], Vec3); 6] = [
        ([4, 5, 6, 7], Vec3::new(0.0, 0.0, 1.0)),
        ([1, 0, 3, 2], Vec3::new(0.0, 0.0, -1.0)),
        ([5, 1, 2, 6], Vec3::new(1.0, 0.0, 0.0)),
        ([0, 4, 7, 3], Vec3::new(-1.0, 0.0, 0.0)),
        ([3, 7, 6, 2], Vec3::new(0.0, 1.0, 0.0)),
        ([0, 1, 5, 4], Vec3::new(0.0, -1.0, 0.0)),
    ];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(12);

    for (quad, normal) in faces {
        let base = vertices.len() as u32;
        let uvs = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        for (k, &ci) in quad.iter().enumerate() {
            vertices.push(Vertex::new(corners[ci], normal, uvs[k]));
        }
        indices.push([base, base + 1, base + 2]);
        indices.push([base, base + 2, base + 3]);
    }

    Mesh::new(vertices, indices)
}

/// Cilindro o cono truncado a lo largo del eje Y, con tapas.
/// Con `top_radius = 0` queda un cono.
pub fn cylinder(bottom_radius: f32, top_radius: f32, height: f32, segments: u32) -> Mesh {
    let segments = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let half = height * 0.5;
    // La normal lateral se inclina segun la conicidad de la pared.
    let slope = (bottom_radius - top_radius) / height;

    for i in 0..=segments {
        let a = i as f32 / segments as f32 * TAU;
        let (s, c) = a.sin_cos();
        let n = Vec3::new(c, slope, s).normalized();
        let u = i as f32 / segments as f32;
        vertices.push(Vertex::new(
            Vec3::new(c * bottom_radius, -half, s * bottom_radius),
            n,
            Vec2::new(u, 0.0),
        ));
        vertices.push(Vertex::new(
            Vec3::new(c * top_radius, half, s * top_radius),
            n,
            Vec2::new(u, 1.0),
        ));
    }

    for i in 0..segments {
        let b0 = i * 2;
        let t0 = i * 2 + 1;
        let b1 = i * 2 + 2;
        let t1 = i * 2 + 3;
        if top_radius > 0.0 {
            indices.push([b0, b1, t0]);
            indices.push([t0, b1, t1]);
        } else {
            indices.push([b0, b1, t0]);
        }
    }

    // Tapa inferior.
    let center_bottom = vertices.len() as u32;
    vertices.push(Vertex::new(
        Vec3::new(0.0, -half, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec2::new(0.5, 0.5),
    ));
    for i in 0..segments {
        let a0 = i as f32 / segments as f32 * TAU;
        let a1 = (i + 1) as f32 / segments as f32 * TAU;
        let p0 = Vec3::new(a0.cos() * bottom_radius, -half, a0.sin() * bottom_radius);
        let p1 = Vec3::new(a1.cos() * bottom_radius, -half, a1.sin() * bottom_radius);
        let base = vertices.len() as u32;
        vertices.push(Vertex::new(p0, Vec3::new(0.0, -1.0, 0.0), Vec2::new(0.0, 0.0)));
        vertices.push(Vertex::new(p1, Vec3::new(0.0, -1.0, 0.0), Vec2::new(1.0, 0.0)));
        indices.push([center_bottom, base + 1, base]);
    }

    if top_radius > 0.0 {
        let center_top = vertices.len() as u32;
        vertices.push(Vertex::new(
            Vec3::new(0.0, half, 0.0),
            Vec3::UP,
            Vec2::new(0.5, 0.5),
        ));
        for i in 0..segments {
            let a0 = i as f32 / segments as f32 * TAU;
            let a1 = (i + 1) as f32 / segments as f32 * TAU;
            let p0 = Vec3::new(a0.cos() * top_radius, half, a0.sin() * top_radius);
            let p1 = Vec3::new(a1.cos() * top_radius, half, a1.sin() * top_radius);
            let base = vertices.len() as u32;
            vertices.push(Vertex::new(p0, Vec3::UP, Vec2::new(0.0, 1.0)));
            vertices.push(Vertex::new(p1, Vec3::UP, Vec2::new(1.0, 1.0)));
            indices.push([center_top, base, base + 1]);
        }
    }

    Mesh::new(vertices, indices)
}

/// Cono apuntando hacia +Y con la base en el origen.
pub fn cone(radius: f32, height: f32, segments: u32) -> Mesh {
    let mut m = cylinder(radius, 0.0, height, segments);
    m.transform(&crate::math::Mat4::translation(Vec3::new(0.0, height * 0.5, 0.0)));
    m
}

/// Bipiramide (dos piramides unidas por la base): la forma de los
/// cristales de la superficie.
pub fn bipyramid(radius: f32, height: f32, sides: u32) -> Mesh {
    let sides = sides.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let top = Vec3::new(0.0, height * 0.5, 0.0);
    let bottom = Vec3::new(0.0, -height * 0.5, 0.0);

    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * TAU;
        let a1 = (i + 1) as f32 / sides as f32 * TAU;
        let p0 = Vec3::new(a0.cos() * radius, 0.0, a0.sin() * radius);
        let p1 = Vec3::new(a1.cos() * radius, 0.0, a1.sin() * radius);

        for (a, b, c) in [(p0, p1, top), (p1, p0, bottom)] {
            let n = (b - a).cross(c - a).normalized();
            let base = vertices.len() as u32;
            vertices.push(Vertex::new(a, n, Vec2::new(0.0, 0.0)));
            vertices.push(Vertex::new(b, n, Vec2::new(1.0, 0.0)));
            vertices.push(Vertex::new(c, n, Vec2::new(0.5, 1.0)));
            indices.push([base, base + 1, base + 2]);
        }
    }

    Mesh::new(vertices, indices)
}
