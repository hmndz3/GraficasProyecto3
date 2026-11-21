//! Shaders de los objetos solidos: la nave, los modelos de superficie y
//! los efectos aditivos de brillo.

use crate::math::{smoothstep, step, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::shaders::common::{
    borde, difuso, direccion_luz, direccion_vista, especular, iluminar, mezcla, AMBIENTE,
};

/// Shader de los objetos sembrados en la superficie de los planetas.
///
/// Un mismo modelo se dibuja muchas veces; el tinte y el parametro por
/// instancia evitan que todas las copias se vean iguales.
pub struct PropShader {
    pub noise: Noise,
    pub base: Vec3,
    pub alterno: Vec3,
    /// Escala del veteado sobre el modelo.
    pub veta: f32,
    pub especular: f32,
    /// Color que emite por si mismo, para cristales y luces.
    pub emision: Vec3,
}

impl FragmentShader for PropShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let v = self.noise.fbm(frag.local * self.veta + Vec3::splat(u.param * 31.0), 4, 2.0, 0.5)
            * 0.5
            + 0.5;
        let albedo = mezcla(self.base, self.alterno, v).mul_elem(u.tint);
        let color = iluminar(albedo, frag.normal, frag, u, 32.0, self.especular);
        let brillo_borde = borde(frag.normal, direccion_vista(frag, u), 2.0);
        Vec4::from_vec3(color + self.emision.mul_elem(u.tint) * (0.4 + brillo_borde * 0.9), 1.0)
    }
}

/// Shader del casco de la nave: paneles metalicos, franja de color y
/// cabina luminosa.
pub struct ShipShader {
    pub noise: Noise,
    pub casco: Vec3,
    pub franja: Vec3,
    pub cabina: Vec3,
}

impl FragmentShader for ShipShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let p = frag.local;

        // Rejilla de paneles: lineas finas donde el casco se junta.
        let rejilla = {
            let gx = (p.x * 6.0).fract().abs();
            let gz = (p.z * 4.0).fract().abs();
            let linea = step(0.94, gx.max(1.0 - gx) * 2.0 - 1.0 + 0.5)
                .max(step(0.94, gz.max(1.0 - gz) * 2.0 - 1.0 + 0.5));
            1.0 - linea * 0.35
        };

        // Franja de color a lo largo del fuselaje.
        let franja = smoothstep(0.06, 0.02, p.y.abs()) * smoothstep(-0.9, 0.2, p.z);

        let sucio = self.noise.fbm(p * 7.0, 3, 2.0, 0.5) * 0.5 + 0.5;
        let mut albedo = mezcla(self.casco * (0.8 + sucio * 0.4), self.franja, franja * 0.85);
        albedo = albedo * rejilla;

        let l = direccion_luz(frag, u);
        let v = direccion_vista(frag, u);
        let d = difuso(frag.normal, l);
        // Metal: especular doble, uno ancho y otro de espejo.
        let s = especular(frag.normal, l, v, 28.0) * 0.5 + especular(frag.normal, l, v, 300.0) * 1.3;
        let mut color = albedo.mul_elem(AMBIENTE * 3.0 + Vec3::splat(d)) + Vec3::splat(s);

        // La cabina emite luz propia: es la parte alta y adelantada.
        let es_cabina = smoothstep(0.16, 0.24, p.y) * smoothstep(-0.2, 0.1, p.z);
        color = color + self.cabina * es_cabina * 1.4;

        Vec4::from_vec3(color, 1.0)
    }
}

/// Brillo aditivo con desvanecido hacia el borde. Lo usan la llama de los
/// motores y los halos.
pub struct GlowShader {
    pub color: Vec3,
    pub potencia: f32,
    pub intensidad: f32,
    /// Si es cierto, el brillo es mas fuerte en el centro en vez de en el
    /// borde.
    pub solido: bool,
}

impl FragmentShader for GlowShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let v = direccion_vista(frag, u);
        let b = borde(frag.normal, v, self.potencia);
        let a = if self.solido { 1.0 - b } else { b };
        Vec4::from_vec3(self.color, (a * self.intensidad).clamp(0.0, 1.0))
    }
}

/// Capa de atmosfera dibujada sobre una esfera algo mayor que el planeta.
/// Solo aporta en el borde y solo del lado iluminado.
pub struct AtmosphereShader {
    pub color: Vec3,
    pub potencia: f32,
    pub intensidad: f32,
}

impl FragmentShader for AtmosphereShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let l = direccion_luz(frag, u);
        let v = direccion_vista(frag, u);
        let anillo = borde(frag.normal, v, self.potencia);
        let lado_luz = frag.normal.dot(l).max(0.0).powf(0.6);
        let a = anillo * (0.12 + lado_luz * 0.95) * self.intensidad;
        Vec4::from_vec3(self.color, a.clamp(0.0, 1.0))
    }
}
