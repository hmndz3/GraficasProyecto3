//! Campos de altura procedurales y deformacion de geometria.
//!
//! Un mismo campo de altura se usa en dos lugares: para desplazar los
//! vertices de la icoesfera antes de rasterizar y, con mas frecuencia, en
//! el fragment shader y en el generador de mapas de normales. Asi el
//! relieve grueso que se ve en la silueta y el detalle fino que se ve de
//! cerca pertenecen al mismo terreno.

use super::mesh::Mesh;
use crate::math::Vec3;
use crate::noise::Noise;

/// Tipo de terreno de un cuerpo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    /// Sin relieve: gigantes gaseosos y la estrella.
    Flat,
    /// Masas continentales amplias con un nivel de mar.
    Continents,
    /// Cordilleras marcadas con ruido de crestas.
    Mountains,
    /// Impactos circulares sobre una base rugosa.
    Craters,
    /// Placas de hielo separadas por grietas.
    IcePlates,
}

/// Parametros del terreno de un cuerpo.
#[derive(Debug, Clone, Copy)]
pub struct Terrain {
    pub kind: TerrainKind,
    /// Cuanto se desplaza la superficie, como fraccion del radio.
    pub amplitude: f32,
    /// Escala del patron: valores altos dan rasgos mas pequeños.
    pub frequency: f32,
    pub seed: u32,
}

impl Terrain {
    pub const fn new(kind: TerrainKind, amplitude: f32, frequency: f32, seed: u32) -> Self {
        Self { kind, amplitude, frequency, seed }
    }

    pub const fn flat(seed: u32) -> Self {
        Self::new(TerrainKind::Flat, 0.0, 1.0, seed)
    }

    pub fn noise(&self) -> Noise {
        Noise::new(self.seed)
    }

    /// Altura en una direccion de la esfera unitaria, en el rango [-1, 1].
    ///
    /// `dir` debe estar normalizado. El resultado no depende del nivel de
    /// subdivision, asi que dos niveles de detalle del mismo planeta
    /// describen la misma montaña.
    pub fn height(&self, dir: Vec3) -> f32 {
        let n = self.noise();
        let p = dir * self.frequency;
        match self.kind {
            TerrainKind::Flat => 0.0,
            TerrainKind::Continents => {
                // Dos escalas: una baja que decide donde hay tierra y otra
                // alta que le da textura a la costa.
                let base = n.fbm(p, 5, 2.0, 0.5);
                let detail = n.fbm(p * 3.7, 4, 2.0, 0.5) * 0.25;
                let h = base + detail;
                // Se aplana todo lo que queda bajo el nivel del mar para
                // que los oceanos se vean planos.
                if h < 0.0 {
                    h * 0.25
                } else {
                    h
                }
            }
            TerrainKind::Mountains => {
                let ridge = n.ridged(p, 6);
                let base = n.fbm(p * 0.7, 3, 2.0, 0.5) * 0.4;
                (ridge * 1.3 - 0.45 + base).clamp(-1.0, 1.0)
            }
            TerrainKind::Craters => {
                let rough = n.fbm(p * 2.3, 4, 2.0, 0.5) * 0.3;
                // El ruido celular da la distancia al centro del crater
                // mas cercano; el perfil convierte esa distancia en un
                // hoyo con borde levantado.
                let d = n.cellular(p * 0.9);
                let rim = (-(d - 0.32) * (d - 0.32) * 26.0).exp();
                let bowl = -(1.0 - (d / 0.32).min(1.0)).powi(2) * 0.8;
                (bowl + rim * 0.55 + rough).clamp(-1.0, 1.0)
            }
            TerrainKind::IcePlates => {
                let d = n.cellular(p * 0.8);
                // Las grietas son las fronteras entre celdas.
                let crack = (1.0 - (d * 6.0).min(1.0)).powi(2);
                let field = n.fbm(p * 2.1, 4, 2.0, 0.5) * 0.35;
                (field - crack * 0.9).clamp(-1.0, 1.0)
            }
        }
    }

    /// Radio desplazado para una direccion dada.
    pub fn radius_at(&self, dir: Vec3, base_radius: f32) -> f32 {
        base_radius * (1.0 + self.height(dir) * self.amplitude)
    }
}

/// Desplaza los vertices de una esfera a lo largo de su normal segun el
/// campo de altura y vuelve a calcular las normales.
///
/// Es la deformacion de geometria: la silueta del planeta deja de ser un
/// circulo perfecto y se ven las montañas contra el fondo.
pub fn displace_sphere(mesh: &mut Mesh, terrain: &Terrain, base_radius: f32) {
    if terrain.amplitude <= 0.0 {
        for v in &mut mesh.vertices {
            v.height = 0.0;
        }
        return;
    }

    for v in &mut mesh.vertices {
        let dir = v.position.normalized();
        let h = terrain.height(dir);
        v.height = h;
        v.position = dir * (base_radius * (1.0 + h * terrain.amplitude));
    }
    mesh.recompute_normals();
    mesh.update_bounds();
}
