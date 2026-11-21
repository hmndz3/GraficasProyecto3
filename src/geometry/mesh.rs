//! Representacion de malla que consume el rasterizador.

use crate::math::{Vec2, Vec3};

/// Vertice en espacio de objeto, tal como sale del generador de geometria.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    /// Canal libre por vertice. La geometria deformada guarda aqui cuanto
    /// se desplazo el vertice respecto del radio base, y los shaders lo
    /// usan para pintar cumbres, costas y fondos marinos.
    pub height: f32,
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self { position, normal, uv, height: 0.0 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<[u32; 3]>,
    /// Radio de la esfera envolvente en espacio de objeto. Lo usa el
    /// descarte por frustum antes de transformar un solo vertice.
    pub bounding_radius: f32,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<[u32; 3]>) -> Self {
        let mut mesh = Self { vertices, indices, bounding_radius: 0.0 };
        mesh.update_bounds();
        mesh
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    pub fn update_bounds(&mut self) {
        let mut r: f32 = 0.0;
        for v in &self.vertices {
            r = r.max(v.position.length());
        }
        self.bounding_radius = r;
    }

    /// Recalcula las normales promediando la normal geometrica de cada
    /// triangulo que toca el vertice. Es el paso que da un sombreado suave
    /// despues de deformar la esfera con ruido.
    pub fn recompute_normals(&mut self) {
        for v in &mut self.vertices {
            v.normal = Vec3::ZERO;
        }
        for tri in &self.indices {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let pa = self.vertices[a].position;
            let pb = self.vertices[b].position;
            let pc = self.vertices[c].position;
            // El area del triangulo queda incluida en la magnitud del
            // producto cruz, asi que los triangulos grandes pesan mas.
            let n = (pb - pa).cross(pc - pa);
            self.vertices[a].normal += n;
            self.vertices[b].normal += n;
            self.vertices[c].normal += n;
        }
        for v in &mut self.vertices {
            v.normal = v.normal.normalized();
        }
    }

    /// Une dos mallas en una sola, desplazando los indices de la segunda.
    pub fn merge(&mut self, other: &Mesh) {
        let offset = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices
            .extend(other.indices.iter().map(|t| [t[0] + offset, t[1] + offset, t[2] + offset]));
        self.update_bounds();
    }

    /// Aplica una transformacion a todos los vertices y la deja "cocida"
    /// en la malla. Sirve para armar los modelos de la nave y de los
    /// objetos de superficie a partir de piezas simples.
    pub fn transform(&mut self, m: &crate::math::Mat4) {
        let nm = m.normal_matrix();
        for v in &mut self.vertices {
            v.position = m.mul_point(v.position);
            v.normal = nm.mul_dir(v.normal).normalized();
        }
        self.update_bounds();
    }
}
