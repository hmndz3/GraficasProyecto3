//! Shader de la estrella: plasma con distorsion de dominio animada.
//!
//! Es el shader animado del proyecto. La tecnica es distorsion de dominio
//! (domain warping): en vez de evaluar el ruido en el punto de la
//! superficie, primero se desplaza ese punto con otro campo de ruido que
//! avanza con el tiempo. El resultado son filamentos que se estiran y se
//! enrollan, en lugar de manchas que solo se desvanecen.
//!
//! No recibe iluminacion: la estrella es la fuente de luz, asi que emite
//! su propio color.

use crate::math::{smoothstep, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::shaders::common::{borde, direccion_vista, mezcla};

pub struct StarShader {
    pub noise: Noise,
    /// Escala del patron sobre la superficie.
    pub scale: f32,
    /// Velocidad del flujo del plasma.
    pub flow: f32,
    /// Cuanto se desplaza el punto de muestreo en cada pasada de
    /// distorsion.
    pub warp_strength: f32,
    pub color_frio: Vec3,
    pub color_medio: Vec3,
    pub color_caliente: Vec3,
    /// Intensidad general de la emision.
    pub intensity: f32,
}

impl StarShader {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Noise::new(seed),
            scale: 3.3,
            flow: 0.09,
            warp_strength: 0.85,
            color_frio: Vec3::new(0.70, 0.070, 0.010),
            color_medio: Vec3::new(2.10, 0.42, 0.030),
            color_caliente: Vec3::new(6.00, 2.30, 0.45),
            intensity: 1.0,
        }
    }
}

impl FragmentShader for StarShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let dir = frag.local.normalized();
        let t = u.time * self.flow;
        let p = dir * self.scale;

        // Primera distorsion: un campo vectorial que arrastra el dominio.
        let q = Vec3::new(
            self.noise.fbm(p + Vec3::new(0.0, t, 0.0), 4, 2.0, 0.5),
            self.noise.fbm(p + Vec3::new(5.2, 1.3 - t, 9.7), 4, 2.0, 0.5),
            self.noise.fbm(p + Vec3::new(2.8, 7.1, 3.4 + t), 4, 2.0, 0.5),
        );

        // Segunda distorsion, aplicada sobre la primera. Encadenarlas es
        // lo que produce los filamentos largos.
        let r = Vec3::new(
            self.noise
                .fbm(p + q * self.warp_strength + Vec3::new(1.7, 9.2, 0.0), 4, 2.0, 0.5),
            self.noise
                .fbm(p + q * self.warp_strength + Vec3::new(8.3, 2.8, t), 4, 2.0, 0.5),
            self.noise
                .fbm(p + q * self.warp_strength + Vec3::new(0.0, 4.4, 6.1), 4, 2.0, 0.5),
        );

        let f = self.noise.fbm(p + r * self.warp_strength, 5, 2.1, 0.52);
        // Se estira el contraste: sin esto el plasma queda en un gris
        // anaranjado plano y no se distinguen los filamentos.
        let v = ((f * 0.5 + 0.5 - 0.5) * 1.9 + 0.5).clamp(0.0, 1.0);

        // Granulacion fina: celdas de conveccion.
        let granos = 1.0 - self.noise.cellular(dir * (self.scale * 7.0) + Vec3::splat(t * 2.0));
        let v = (v * 0.86 + granos * 0.14).clamp(0.0, 1.0);

        // Manchas solares: zonas donde el campo cae por debajo de un
        // umbral se apagan.
        let mancha = smoothstep(0.18, 0.34, self.noise.fbm(p * 0.55 - Vec3::splat(t * 0.4), 3, 2.0, 0.5) * 0.5 + 0.5);

        let base = if v < 0.5 {
            mezcla(self.color_frio, self.color_medio, v * 2.0)
        } else {
            mezcla(self.color_medio, self.color_caliente, (v - 0.5) * 2.0)
        };

        let mut color = base * mezcla(Vec3::splat(0.35), Vec3::ONE, mancha).x;

        // Borde mas brillante: el limbo de la estrella.
        let vista = direccion_vista(frag, u);
        let halo = borde(frag.normal, vista, 2.2);
        color = color + self.color_caliente * halo * 0.55;

        Vec4::from_vec3(color * self.intensity, 1.0)
    }
}

/// Corona exterior de la estrella, dibujada con mezcla aditiva sobre una
/// esfera un poco mas grande. Tambien se anima con el tiempo.
pub struct CoronaShader {
    pub noise: Noise,
    pub color: Vec3,
    pub flow: f32,
    /// Cuanto mas grande es la esfera de la corona que la estrella.
    pub escala: f32,
}


impl FragmentShader for CoronaShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let dir = frag.local.normalized();
        let t = u.time * self.flow;
        let vista = direccion_vista(frag, u);

        // La corona se dibuja sobre una esfera mas grande que la
        // estrella. El coseno entre la normal y la vista dice a que
        // radio aparente cae cada pixel: vale `limbo` justo en el borde
        // del disco de la estrella y cero en la silueta de la esfera
        // exterior. Se quiere un halo que nazca en el limbo y se apague
        // hacia afuera, y que no aporte nada por delante de la estrella.
        let c = frag.normal.dot(vista).abs().clamp(0.0, 1.0);
        let limbo = (1.0 - 1.0 / (self.escala * self.escala)).max(0.0).sqrt();
        let hacia_afuera = (c / limbo.max(1e-3)).clamp(0.0, 1.0).powf(2.6);
        let delante = 1.0 - smoothstep(limbo, limbo + 0.09, c);

        let lenguas =
            self.noise.fbm(dir * 3.4 + Vec3::new(t, -t * 0.6, t * 0.3), 4, 2.2, 0.5) * 0.5 + 0.5;
        let alpha = (hacia_afuera * delante * (0.35 + lenguas * 0.85)).clamp(0.0, 1.0);

        Vec4::from_vec3(self.color, alpha * 0.85)
    }
}
