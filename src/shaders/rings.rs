//! Shader del sistema de anillos.
//!
//! La coordenada u del anillo va de 0 en el borde interno a 1 en el
//! externo. Sobre ella se apilan varias frecuencias de ruido para sacar
//! bandas densas y huecos, y la densidad resultante se usa como cobertura
//! en la mezcla aditiva: donde no hay material, se ve el fondo.

use crate::math::{smoothstep, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::shaders::common::{direccion_luz, mezcla};

pub struct RingShader {
    pub noise: Noise,
    pub color_interno: Vec3,
    pub color_externo: Vec3,
    /// Cuantas bandas finas caben a lo ancho del anillo.
    pub frecuencia: f32,
    /// Radio relativo del hueco principal.
    pub hueco: f32,
    pub densidad: f32,
}

impl FragmentShader for RingShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let t = frag.uv.x.clamp(0.0, 1.0);

        // Tres escalas de bandas superpuestas.
        let b1 = self.noise.value(Vec3::new(t * self.frecuencia, 0.0, 0.0)) * 0.5 + 0.5;
        let b2 = self.noise.value(Vec3::new(t * self.frecuencia * 3.7, 11.0, 0.0)) * 0.5 + 0.5;
        let b3 = self.noise.value(Vec3::new(t * self.frecuencia * 9.1, 23.0, 0.0)) * 0.5 + 0.5;
        let banda = (b1 * 0.55 + b2 * 0.3 + b3 * 0.15).clamp(0.0, 1.0);

        // Hueco limpio en una posicion fija y bordes desvanecidos.
        let gap = smoothstep(0.02, 0.06, (t - self.hueco).abs());
        let borde_interno = smoothstep(0.0, 0.07, t);
        let borde_externo = 1.0 - smoothstep(0.9, 1.0, t);

        let cobertura = (banda * gap * borde_interno * borde_externo * self.densidad).clamp(0.0, 1.0);

        // El material se ilumina de forma plana: las particulas dispersan
        // la luz en todas direcciones.
        let l = direccion_luz(frag, u);
        let luz = frag.normal.dot(l).abs().max(0.25);

        let color = mezcla(self.color_interno, self.color_externo, t) * luz;
        Vec4::from_vec3(color, cobertura)
    }
}
