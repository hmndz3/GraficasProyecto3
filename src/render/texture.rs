//! Texturas en memoria y generacion de mapas de normales.
//!
//! No se cargan imagenes de disco: los mapas se calculan al arrancar a
//! partir del mismo campo de altura que deforma la geometria. El
//! muestreo bilineal y el paso de tangentes estan escritos aqui, que es lo
//! que convierte esto en mapeo de normales hecho dentro del renderizador y
//! no en un detalle de la libreria de ventanas.

use crate::geometry::terrain::Terrain;
use crate::math::{Vec2, Vec3, PI, TAU};
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Vec3>,
}

impl Texture {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![Vec3::ZERO; width * height] }
    }

    #[inline]
    fn texel(&self, x: i64, y: i64) -> Vec3 {
        // Horizontal se repite (la textura da la vuelta al planeta),
        // vertical se recorta en los polos.
        let w = self.width as i64;
        let h = self.height as i64;
        let xi = ((x % w) + w) % w;
        let yi = y.clamp(0, h - 1);
        self.data[(yi * w + xi) as usize]
    }

    /// Muestreo bilineal con coordenadas en [0, 1].
    pub fn sample(&self, uv: Vec2) -> Vec3 {
        let x = uv.x * self.width as f32 - 0.5;
        let y = uv.y * self.height as f32 - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (xi, yi) = (x0 as i64, y0 as i64);

        let c00 = self.texel(xi, yi);
        let c10 = self.texel(xi + 1, yi);
        let c01 = self.texel(xi, yi + 1);
        let c11 = self.texel(xi + 1, yi + 1);

        let a = c00.lerp(c10, fx);
        let b = c01.lerp(c11, fx);
        a.lerp(b, fy)
    }
}

/// Direccion sobre la esfera que corresponde a una coordenada
/// equirectangular. Es la inversa del mapeo que usa `sphere::icosphere`.
pub fn uv_to_direction(u: f32, v: f32) -> Vec3 {
    let theta = (u - 0.5) * TAU;
    let phi = (0.5 - v) * PI;
    let cp = phi.cos();
    Vec3::new(cp * theta.cos(), phi.sin(), cp * theta.sin())
}

/// Genera un mapa de normales en espacio tangente a partir del campo de
/// altura del terreno.
///
/// La pendiente se obtiene por diferencias finitas sobre la esfera: se
/// evalua la altura en dos direcciones vecinas por eje tangente y la
/// diferencia da el gradiente. El vector resultante se guarda codificado
/// en [0, 1] como cualquier mapa de normales.
pub fn generate_normal_map(terrain: &Terrain, width: usize, height: usize, strength: f32) -> Texture {
    let mut tex = Texture::new(width, height);
    let w = width;
    let h = height;

    tex.data
        .par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row)| {
            let v = (y as f32 + 0.5) / h as f32;
            for (x, texel) in row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) / w as f32;
                let n = uv_to_direction(u, v);

                // Base tangente de la parametrizacion equirectangular.
                let tangent = Vec3::UP.cross(n).normalized();
                let tangent = if tangent == Vec3::ZERO {
                    Vec3::new(1.0, 0.0, 0.0)
                } else {
                    tangent
                };
                let bitangent = n.cross(tangent);

                // Paso angular pequeño, proporcional a la resolucion.
                let eps = TAU / w as f32;
                let hu1 = terrain.height((n + tangent * eps).normalized());
                let hu0 = terrain.height((n - tangent * eps).normalized());
                let hv1 = terrain.height((n + bitangent * eps).normalized());
                let hv0 = terrain.height((n - bitangent * eps).normalized());

                let du = (hu1 - hu0) / (2.0 * eps);
                let dv = (hv1 - hv0) / (2.0 * eps);

                // En espacio tangente la normal perturbada es
                // (-du, -dv, 1) normalizada.
                let nt = Vec3::new(-du * strength, -dv * strength, 1.0).normalized();
                *texel = nt * 0.5 + Vec3::splat(0.5);
            }
        });

    tex
}

/// Decodifica un texel de mapa de normales y lo lleva a espacio de mundo
/// con la base TBN construida sobre la normal interpolada.
#[inline]
pub fn apply_normal_map(normal: Vec3, encoded: Vec3, strength: f32) -> Vec3 {
    let nt = (encoded * 2.0 - Vec3::ONE).normalized();
    let nt = Vec3::new(nt.x * strength, nt.y * strength, nt.z).normalized();

    let n = normal.normalized();
    let mut t = Vec3::UP.cross(n);
    if t.length_squared() < 1e-8 {
        t = Vec3::new(1.0, 0.0, 0.0);
    }
    let t = t.normalized();
    let b = n.cross(t);

    (t * nt.x + b * nt.y + n * nt.z).normalized()
}

/// Igual que `apply_normal_map`, pero con la tangente dada por quien
/// llama.
///
/// Es la version que usan los planetas: la tangente se construye en
/// espacio de objeto y se lleva a mundo con la matriz de normales, de modo
/// que el relieve gira junto con el planeta en vez de quedarse pegado a la
/// pantalla. La tangente se reortogonaliza contra la normal interpolada
/// (Gram-Schmidt) porque la geometria deformada ya inclino esa normal.
#[inline]
pub fn apply_normal_map_tbn(
    normal_world: Vec3,
    tangent_world: Vec3,
    encoded: Vec3,
    strength: f32,
) -> Vec3 {
    let n = normal_world.normalized();
    let t = tangent_world - n * n.dot(tangent_world);
    if t.length_squared() < 1e-10 {
        return n;
    }
    let t = t.normalized();
    let b = n.cross(t);

    let nt = (encoded * 2.0 - Vec3::ONE).normalized();
    let nt = Vec3::new(nt.x * strength, nt.y * strength, nt.z).normalized();

    (t * nt.x + b * nt.y + n * nt.z).normalized()
}
