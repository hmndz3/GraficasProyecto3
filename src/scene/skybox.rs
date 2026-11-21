//! Fondo estrellado.
//!
//! Tiene dos partes. La nebulosa es una textura equirectangular que se
//! calcula una sola vez al arrancar y despues se muestrea con la direccion
//! del rayo de cada pixel, lo que evita evaluar ruido fractal un millon de
//! veces por cuadro. Las estrellas son puntos con posicion fija en el
//! cielo que se proyectan con la misma camara que el resto de la escena,
//! asi que el campo estelar se mantiene quieto mientras la nave gira.

use crate::math::{Mat4, Vec2, Vec3, Vec4, PI, TAU};
use crate::noise::{Noise, Rng};
use crate::render::framebuffer::{pack_color, Framebuffer};
use crate::render::raster::splat_point;
use crate::render::texture::{uv_to_direction, Texture};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct Star {
    pub direction: Vec3,
    pub color: Vec3,
    /// Brillo relativo, entre 0 y 1.
    pub magnitude: f32,
}

pub struct Skybox {
    pub nebula: Texture,
    pub stars: Vec<Star>,
}

impl Skybox {
    /// Genera el cielo. `seed` fija tanto la nebulosa como las estrellas.
    pub fn new(seed: u32, star_count: usize) -> Self {
        let nebula = generar_nebulosa(seed, 256, 128);

        let mut rng = Rng::new(seed ^ 0x9e37_79b9);
        let mut stars = Vec::with_capacity(star_count);
        for _ in 0..star_count {
            let direction = rng.unit_sphere();
            // Casi todas las estrellas son debiles y unas pocas brillan:
            // elevar a una potencia alta produce esa distribucion.
            let m = rng.next_f32().powf(3.2);
            // Temperatura de color: de anaranjado a azulado.
            let t = rng.next_f32();
            let color = Vec3::new(
                0.75 + 0.35 * (1.0 - t),
                0.78 + 0.18 * (1.0 - (t - 0.5).abs() * 2.0),
                0.80 + 0.35 * t,
            );
            stars.push(Star { direction, color, magnitude: m });
        }

        Self { nebula, stars }
    }

    /// Pinta el fondo en todo el framebuffer y limpia la profundidad.
    ///
    /// Como el cielo esta infinitamente lejos, no participa en la prueba
    /// de profundidad: se dibuja primero y todo lo demas lo tapa.
    pub fn draw_background(
        &self,
        fb: &mut Framebuffer,
        camera_basis: (Vec3, Vec3, Vec3),
        fov_y: f32,
    ) {
        let (right, up, forward) = camera_basis;
        let w = fb.width;
        let h = fb.height;
        let aspect = w as f32 / h as f32;
        let tan_half = (fov_y * 0.5).tan();
        let nebula = &self.nebula;

        fb.depth.fill(crate::render::framebuffer::DEPTH_CLEAR);
        fb.color
            .par_chunks_mut(w)
            .enumerate()
            .for_each(|(y, fila)| {
                let ndc_y = 1.0 - 2.0 * (y as f32 + 0.5) / h as f32;
                for (x, px) in fila.iter_mut().enumerate() {
                    let ndc_x = 2.0 * (x as f32 + 0.5) / w as f32 - 1.0;
                    let dir = (forward
                        + right * (ndc_x * tan_half * aspect)
                        + up * (ndc_y * tan_half))
                        .normalized();

                    let u = 0.5 + dir.z.atan2(dir.x) / TAU;
                    let v = 0.5 - dir.y.clamp(-1.0, 1.0).asin() / PI;
                    *px = pack_color(nebula.sample(Vec2::new(u, v)));
                }
            });
    }

    /// Proyecta y dibuja las estrellas.
    pub fn draw_stars(&self, fb: &mut Framebuffer, view_projection: &Mat4) {
        let w = fb.width as f32;
        let h = fb.height as f32;

        for s in &self.stars {
            // Las estrellas se tratan como puntos muy lejanos: se
            // proyectan como direcciones, sin traslacion de camara.
            let clip = view_projection.mul_vec4(Vec4::from_vec3(s.direction * 1000.0, 1.0));
            if clip.w <= 1e-4 {
                continue;
            }
            let inv_w = 1.0 / clip.w;
            let ndc_x = clip.x * inv_w;
            let ndc_y = clip.y * inv_w;
            if !(-1.05..=1.05).contains(&ndc_x) || !(-1.05..=1.05).contains(&ndc_y) {
                continue;
            }
            let px = ((ndc_x * 0.5 + 0.5) * w) as i64;
            let py = ((0.5 - ndc_y * 0.5) * h) as i64;

            let brillo = 0.06 + s.magnitude * 1.1;
            let radio = if s.magnitude > 0.93 { 1 } else { 0 };
            splat_point(fb, px, py, radio, s.color * brillo);
        }
    }
}

/// Nebulosa equirectangular: varias capas de ruido fractal en distintos
/// colores sobre un fondo casi negro.
fn generar_nebulosa(seed: u32, width: usize, height: usize) -> Texture {
    let n1 = Noise::new(seed);
    let n2 = Noise::new(seed ^ 0x1234_5678);
    let n3 = Noise::new(seed ^ 0x8765_4321);

    let mut tex = Texture::new(width, height);
    tex.data
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, fila)| {
            let v = (y as f32 + 0.5) / height as f32;
            for (x, texel) in fila.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) / width as f32;
                let d = uv_to_direction(u, v);

                let a = (n1.fbm(d * 1.6, 5, 2.2, 0.55) * 0.5 + 0.5).powf(3.0);
                let b = (n2.fbm(d * 2.9 + Vec3::splat(13.0), 5, 2.0, 0.5) * 0.5 + 0.5).powf(4.0);
                let c = (n3.fbm(d * 0.9 - Vec3::splat(7.0), 4, 2.0, 0.5) * 0.5 + 0.5).powf(2.5);

                // Fondo frio con dos nebulosas de distinto color.
                let base = Vec3::new(0.0020, 0.0028, 0.0060);
                let morada = Vec3::new(0.038, 0.012, 0.062) * a;
                let turquesa = Vec3::new(0.005, 0.034, 0.044) * b;
                let polvo = Vec3::new(0.020, 0.011, 0.006) * c * 0.5;

                *texel = base + morada + turquesa + polvo;
            }
        });
    tex
}
