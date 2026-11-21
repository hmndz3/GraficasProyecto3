//! Cuerpos del sistema: estrella, planetas y lunas.
//!
//! Cada cuerpo guarda su orbita, su giro propio, su terreno, sus niveles
//! de detalle y el shader con el que se pinta. Las posiciones se recalculan
//! en cada cuadro a partir del tiempo simulado, nunca del numero de
//! cuadros dibujados.

use std::sync::Arc;

use crate::geometry::mesh::Mesh;
use crate::geometry::terrain::Terrain;
use crate::math::{Mat4, Vec3, TAU};
use crate::render::pipeline::FragmentShader;
use crate::render::texture::Texture;

/// Un objeto colocado sobre la superficie de un planeta.
#[derive(Debug, Clone, Copy)]
pub struct PropInstance {
    /// Cual de los cuatro modelos se usa.
    pub kind: usize,
    /// Direccion desde el centro del planeta, normalizada.
    pub direction: Vec3,
    /// Distancia al centro: el radio del terreno en esa direccion.
    pub radius: f32,
    pub scale: f32,
    /// Giro alrededor de la normal de la superficie.
    pub yaw: f32,
    pub tint: Vec3,
    pub param: f32,
}

impl PropInstance {
    /// Matriz de modelo del objeto en espacio del planeta.
    ///
    /// La base se arma con la normal de la superficie como eje vertical,
    /// asi que el objeto queda parado sobre la curva del planeta y no
    /// atravesado ni flotando. Como la matriz se multiplica despues por la
    /// del planeta, el objeto acompaña la rotacion del cuerpo.
    pub fn local_matrix(&self) -> Mat4 {
        let up = self.direction;
        // Eje auxiliar que no sea paralelo a la normal.
        let aux = if up.y.abs() > 0.95 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::UP
        };
        let right = aux.cross(up).normalized();
        let forward = up.cross(right);

        let base = Mat4::from_basis(right, up, forward, up * self.radius);
        base * Mat4::rotation_y(self.yaw) * Mat4::uniform_scale(self.scale)
    }
}

/// Parametros de la orbita de un cuerpo alrededor de su padre.
#[derive(Debug, Clone, Copy)]
pub struct Orbit {
    pub radius: f32,
    /// Radianes por segundo.
    pub speed: f32,
    pub phase: f32,
    /// Inclinacion respecto del plano ecliptico, en radianes. Los
    /// planetas la tienen en cero: todos comparten el mismo plano.
    pub inclination: f32,
}

impl Orbit {
    pub fn position(&self, time: f32) -> Vec3 {
        let a = self.phase + self.speed * time;
        let (s, c) = a.sin_cos();
        let p = Vec3::new(c * self.radius, 0.0, s * self.radius);
        if self.inclination.abs() < 1e-6 {
            p
        } else {
            Mat4::rotation_x(self.inclination).mul_point(p)
        }
    }

    pub fn period(&self) -> f32 {
        if self.speed.abs() < 1e-9 {
            f32::INFINITY
        } else {
            TAU / self.speed.abs()
        }
    }
}

/// Capa de atmosfera opcional.
#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    pub color: Vec3,
    /// Grosor como fraccion del radio del planeta.
    pub thickness: f32,
    pub intensity: f32,
}

pub struct Body {
    pub name: String,
    pub radius: f32,
    pub orbit: Orbit,
    /// Indice del cuerpo alrededor del cual orbita. `None` para la
    /// estrella.
    pub parent: Option<usize>,
    /// Radianes por segundo de giro sobre su propio eje.
    pub spin_speed: f32,
    /// Inclinacion del eje de giro, en radianes.
    pub axial_tilt: f32,
    pub terrain: Terrain,
    /// Mallas de mayor a menor detalle.
    pub lods: Vec<Mesh>,
    pub shader: Box<dyn FragmentShader + Send + Sync>,
    pub normal_map: Option<Arc<Texture>>,
    pub atmosphere: Option<Atmosphere>,
    pub rings: Option<RingSystem>,
    pub props: Vec<PropInstance>,
    /// Las instancias ya fusionadas en una malla por tipo, en espacio del
    /// planeta. Dibujar una sola malla por tipo evita cientos de llamadas
    /// de dibujo diminutas.
    pub prop_batches: Vec<PropBatch>,
    /// Masa relativa, solo para la atraccion sobre la nave.
    pub mass: f32,
    /// Si es cierto, el cuerpo emite luz y no se le aplica sombreado.
    pub emissive: bool,

    // Estado recalculado cada cuadro.
    pub position: Vec3,
    pub spin: f32,
}

/// Todas las copias de un mismo modelo de superficie, fusionadas.
pub struct PropBatch {
    pub kind: usize,
    pub mesh: Mesh,
    pub tint: Vec3,
}

pub struct RingSystem {
    pub mesh: Mesh,
    pub shader: Box<dyn FragmentShader + Send + Sync>,
    pub outer_radius: f32,
}

impl Body {
    /// Matriz de modelo del cuerpo: posicion orbital, inclinacion del eje
    /// y giro propio.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::translation(self.position)
            * Mat4::rotation_z(self.axial_tilt)
            * Mat4::rotation_y(self.spin)
    }

    /// Matriz que solo lleva la posicion y la inclinacion, sin el giro.
    /// Los anillos no giran con el planeta.
    pub fn frame_matrix(&self) -> Mat4 {
        Mat4::translation(self.position) * Mat4::rotation_z(self.axial_tilt)
    }

    /// Radio que incluye el relieve mas alto. Se usa para el descarte y
    /// para la colision.
    pub fn outer_radius(&self) -> f32 {
        self.radius * (1.0 + self.terrain.amplitude.abs())
    }

    /// Altura del terreno en una direccion dada del espacio del planeta.
    pub fn surface_radius(&self, dir_local: Vec3) -> f32 {
        self.terrain.radius_at(dir_local.normalized(), self.radius)
    }

    /// Elige el nivel de detalle segun el tamaño aparente del cuerpo.
    ///
    /// Se compara el radio con la distancia a la camara: si el planeta
    /// ocupa poco en pantalla, no tiene sentido rasterizar miles de
    /// triangulos.
    pub fn lod_index(&self, camera: Vec3, habilitado: bool) -> usize {
        if !habilitado || self.lods.len() <= 1 {
            return 0;
        }
        let distancia = (camera - self.position).length().max(1e-3);
        let tamano_aparente = self.outer_radius() / distancia;

        if tamano_aparente > 0.30 {
            0
        } else if tamano_aparente > 0.12 {
            1.min(self.lods.len() - 1)
        } else if tamano_aparente > 0.045 {
            2.min(self.lods.len() - 1)
        } else {
            self.lods.len() - 1
        }
    }
}
