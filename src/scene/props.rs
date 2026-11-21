//! Distribucion procedural de objetos sobre la superficie de un planeta.
//!
//! No hay posiciones escritas a mano. Se siembran candidatos sobre la
//! esfera con un generador de semilla fija, un campo de ruido decide donde
//! hay "bosque" y donde no, y un rechazo por distancia angular minima
//! evita que dos objetos se encimen. Con la misma semilla el resultado es
//! identico en cualquier corrida.

use crate::geometry::terrain::Terrain;
use crate::math::{smoothstep, Vec3, TAU};
use crate::noise::{Noise, Rng};
use crate::scene::body::PropInstance;

/// Reglas de siembra de un planeta.
#[derive(Debug, Clone)]
pub struct PropRecipe {
    pub seed: u32,
    /// Cuantos candidatos se prueban. Los que caen fuera del bioma o
    /// demasiado cerca de otro se descartan.
    pub attempts: usize,
    /// Cuantos objetos se aceptan como maximo.
    pub max_instances: usize,
    /// Separacion angular minima entre dos objetos, en radianes.
    pub min_separation: f32,
    /// Escala del campo de ruido que define los bioma.
    pub cluster_scale: f32,
    /// Umbral del campo: valores altos dejan menos zonas pobladas.
    pub cluster_threshold: f32,
    /// Altura minima del terreno para aceptar un objeto. Sirve para no
    /// sembrar arboles dentro del oceano.
    pub min_height: f32,
    /// Peso relativo de cada uno de los cuatro modelos.
    pub weights: [f32; 4],
    /// Rango de escala de los objetos.
    pub scale_range: (f32, f32),
    pub tints: [Vec3; 4],
}

impl Default for PropRecipe {
    fn default() -> Self {
        Self {
            seed: 1,
            attempts: 4000,
            max_instances: 160,
            min_separation: 0.085,
            cluster_scale: 2.4,
            cluster_threshold: 0.12,
            min_height: -1.0,
            weights: [1.0, 1.0, 1.0, 1.0],
            scale_range: (0.7, 1.4),
            tints: [Vec3::ONE; 4],
        }
    }
}

/// Siembra los objetos de un planeta.
pub fn scatter(recipe: &PropRecipe, terrain: &Terrain, planet_radius: f32) -> Vec<PropInstance> {
    let mut rng = Rng::new(recipe.seed.wrapping_mul(2654435761).wrapping_add(12345));
    let campo = Noise::new(recipe.seed ^ 0x5bf0_3635);

    let peso_total: f32 = recipe.weights.iter().sum();
    let mut aceptados: Vec<PropInstance> = Vec::with_capacity(recipe.max_instances);
    // Coseno de la separacion minima: comparar productos punto es mas
    // barato que calcular angulos.
    let cos_min = recipe.min_separation.cos();

    for _ in 0..recipe.attempts {
        if aceptados.len() >= recipe.max_instances {
            break;
        }
        let dir = rng.unit_sphere();

        // El campo de ruido decide si esta direccion pertenece a una zona
        // poblada.
        let densidad = campo.fbm(dir * recipe.cluster_scale, 4, 2.1, 0.55);
        if densidad < recipe.cluster_threshold {
            continue;
        }

        let h = terrain.height(dir);
        if h < recipe.min_height {
            continue;
        }

        // Rechazo por cercania: equivale a un muestreo de Poisson sobre la
        // esfera.
        if aceptados.iter().any(|p| p.direction.dot(dir) > cos_min) {
            continue;
        }

        // El tipo se escoge con los pesos, sesgado por la altura: lo alto
        // y frio favorece cristales y roca.
        let r = rng.next_f32() * peso_total;
        let mut acc = 0.0;
        let mut kind = 0usize;
        for (i, w) in recipe.weights.iter().enumerate() {
            acc += w;
            if r <= acc {
                kind = i;
                break;
            }
        }
        // En las cumbres se cambian arboles por roca.
        if kind == 0 && h > 0.55 {
            kind = 1;
        }

        // Mas densidad de ruido, objetos un poco mas grandes.
        let bono = smoothstep(recipe.cluster_threshold, 1.0, densidad) * 0.3;
        let escala = rng.range(recipe.scale_range.0, recipe.scale_range.1) * (1.0 + bono);

        let radio = terrain.radius_at(dir, planet_radius);
        // Se hunde un poco la base para que no se vea el borde del modelo
        // flotando sobre el relieve.
        let radio = radio - 0.012 * planet_radius * escala;

        let variacion = rng.range(0.82, 1.18);
        aceptados.push(PropInstance {
            kind,
            direction: dir,
            radius: radio,
            scale: escala * planet_radius * 0.105,
            yaw: rng.range(0.0, TAU),
            tint: recipe.tints[kind] * variacion,
            param: rng.next_f32(),
        });
    }

    aceptados
}
