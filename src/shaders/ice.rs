//! Shader de mundo helado: placas separadas por grietas.
//!
//! El patron sale del ruido celular. La distancia al centro de celda mas
//! cercano da el interior de la placa; las fronteras entre celdas son las
//! grietas, que se pintan oscuras y hundidas. El hielo refleja mucho, asi
//! que el especular es ancho y fuerte.

use std::sync::Arc;

use crate::math::{smoothstep, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::render::texture::{apply_normal_map_tbn, Texture};
use crate::shaders::common::{
    borde, difuso, direccion_luz, direccion_vista, especular, mezcla, tangente_mundo, AMBIENTE,
};

pub struct IceShader {
    pub noise: Noise,
    pub normal_map: Arc<Texture>,
    pub normal_strength: f32,
    /// Escala del mosaico de placas.
    pub escala_placas: f32,
    pub hielo_claro: Vec3,
    pub hielo_sombra: Vec3,
    pub color_grieta: Vec3,
    pub color_atmosfera: Vec3,
}

impl FragmentShader for IceShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let dir = frag.local.normalized();
        let n = apply_normal_map_tbn(
            frag.normal,
            tangente_mundo(frag, u),
            self.normal_map.sample(frag.uv),
            self.normal_strength,
        );

        let d = self.noise.cellular(dir * self.escala_placas);
        // Cerca de cero estamos sobre una frontera de celda.
        let grieta = 1.0 - smoothstep(0.0, 0.16, d);
        // Dos escalas mas de grietas finas.
        let fina = 1.0 - smoothstep(0.0, 0.07, self.noise.cellular(dir * self.escala_placas * 3.1));

        let brillo = self.noise.fbm(dir * 9.0, 4, 2.0, 0.5) * 0.5 + 0.5;
        let hielo = mezcla(self.hielo_sombra, self.hielo_claro, brillo);
        let albedo = mezcla(hielo, self.color_grieta, (grieta * 0.9 + fina * 0.45).min(1.0));

        let l = direccion_luz(frag, u);
        let v = direccion_vista(frag, u);
        let dif = difuso(n, l);
        // Dos lobulos especulares: uno ancho del hielo rugoso y otro muy
        // concentrado de las superficies pulidas.
        let s = especular(n, l, v, 16.0) * 0.35 + especular(n, l, v, 220.0) * 1.1;
        let s = s * (1.0 - grieta * 0.8);

        let color = albedo.mul_elem(AMBIENTE + Vec3::splat(dif)) + Vec3::splat(s);
        let halo = borde(frag.normal, v, 3.2) * dif.max(0.05);
        Vec4::from_vec3(color + self.color_atmosfera * halo * 0.7, 1.0)
    }
}
