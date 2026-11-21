//! Generacion y subdivision procedural de esferas.
//!
//! Los planetas no se cargan de ningun archivo: se construyen aqui. Se
//! parte de un icosaedro de 20 caras y se subdivide tantas veces como pida
//! el nivel de detalle. A diferencia de una esfera de latitud y longitud,
//! el icosaedro reparte los triangulos de forma pareja y no acumula
//! vertices en los polos.
//!
//! Esta etapa corre en el pipeline por software antes de rasterizar.

use super::mesh::{Mesh, Vertex};
use crate::math::{Vec2, Vec3, PI, TAU};
use std::collections::HashMap;

/// Vertices del icosaedro regular, ya normalizados al radio 1.
fn icosahedron() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    // Proporcion aurea: con ella los 12 vertices caen en tres rectangulos
    // ortogonales y todas las aristas miden lo mismo.
    let t = (1.0 + 5.0f32.sqrt()) * 0.5;

    let raw = [
        Vec3::new(-1.0, t, 0.0),
        Vec3::new(1.0, t, 0.0),
        Vec3::new(-1.0, -t, 0.0),
        Vec3::new(1.0, -t, 0.0),
        Vec3::new(0.0, -1.0, t),
        Vec3::new(0.0, 1.0, t),
        Vec3::new(0.0, -1.0, -t),
        Vec3::new(0.0, 1.0, -t),
        Vec3::new(t, 0.0, -1.0),
        Vec3::new(t, 0.0, 1.0),
        Vec3::new(-t, 0.0, -1.0),
        Vec3::new(-t, 0.0, 1.0),
    ];

    let verts: Vec<Vec3> = raw.iter().map(|v| v.normalized()).collect();

    let faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    (verts, faces)
}

/// Clave de una arista, siempre con el indice menor primero para que los
/// dos triangulos que la comparten reutilicen el mismo punto medio.
fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Subdivide cada triangulo en cuatro y vuelve a proyectar los puntos
/// nuevos sobre la esfera. El cache de aristas evita duplicar vertices y
/// mantiene la malla cerrada.
pub fn subdivide(verts: &mut Vec<Vec3>, faces: Vec<[u32; 3]>) -> Vec<[u32; 3]> {
    let mut cache: HashMap<(u32, u32), u32> = HashMap::with_capacity(faces.len() * 3);
    let mut out = Vec::with_capacity(faces.len() * 4);

    for f in faces {
        let mut mid = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
            let key = edge_key(a, b);
            if let Some(&i) = cache.get(&key) {
                return i;
            }
            let p = ((verts[a as usize] + verts[b as usize]) * 0.5).normalized();
            verts.push(p);
            let i = (verts.len() - 1) as u32;
            cache.insert(key, i);
            i
        };

        let ab = mid(f[0], f[1], verts);
        let bc = mid(f[1], f[2], verts);
        let ca = mid(f[2], f[0], verts);

        out.push([f[0], ab, ca]);
        out.push([f[1], bc, ab]);
        out.push([f[2], ca, bc]);
        out.push([ab, bc, ca]);
    }
    out
}

/// Construye una icoesfera de radio `radius` con `subdivisions` pasos.
///
/// | subdivisiones | triangulos |
/// |---|---|
/// | 0 | 20 |
/// | 1 | 80 |
/// | 2 | 320 |
/// | 3 | 1280 |
/// | 4 | 5120 |
pub fn icosphere(radius: f32, subdivisions: u32) -> Mesh {
    let (mut verts, mut faces) = icosahedron();
    for _ in 0..subdivisions {
        faces = subdivide(&mut verts, faces);
    }

    let vertices: Vec<Vertex> = verts
        .iter()
        .map(|&n| {
            // Coordenadas esfericas: u da la vuelta en el ecuador y v va
            // del polo sur al norte.
            let u = 0.5 + n.z.atan2(n.x) / TAU;
            let v = 0.5 - n.y.clamp(-1.0, 1.0).asin() / PI;
            Vertex::new(n * radius, n, Vec2::new(u, v))
        })
        .collect();

    Mesh::new(vertices, faces)
}

/// Anillo plano en el plano XZ, para los sistemas de anillos.
pub fn ring(inner: f32, outer: f32, segments: u32) -> Mesh {
    let segments = segments.max(8);
    let mut vertices = Vec::with_capacity(segments as usize * 2);
    let mut indices = Vec::with_capacity(segments as usize * 2);

    for i in 0..segments {
        let a = i as f32 / segments as f32 * TAU;
        let (s, c) = a.sin_cos();
        // u recorre el ancho del anillo: 0 en el borde interno, 1 en el
        // externo. Los shaders lo usan para dibujar las bandas.
        vertices.push(Vertex::new(
            Vec3::new(c * inner, 0.0, s * inner),
            Vec3::UP,
            Vec2::new(0.0, i as f32 / segments as f32),
        ));
        vertices.push(Vertex::new(
            Vec3::new(c * outer, 0.0, s * outer),
            Vec3::UP,
            Vec2::new(1.0, i as f32 / segments as f32),
        ));
    }

    for i in 0..segments {
        let i0 = (i * 2) % (segments * 2);
        let i1 = (i * 2 + 1) % (segments * 2);
        let i2 = (i * 2 + 2) % (segments * 2);
        let i3 = (i * 2 + 3) % (segments * 2);
        indices.push([i0, i2, i1]);
        indices.push([i1, i2, i3]);
    }

    Mesh::new(vertices, indices)
}
