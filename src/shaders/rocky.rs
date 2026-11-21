//! Shader de planeta rocoso.
//!
//! Combina tres cosas: el mapa de normales generado a partir del mismo
//! terreno que deformo la geometria, una rampa de color por altura y un
//! polvo fino que rompe la uniformidad del albedo. El relieve que se ve de
//! cerca viene del mapa de normales, no de mas triangulos.

use std::sync::Arc;

use crate::math::{smoothstep, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::render::texture::{apply_normal_map_tbn, Texture};
use crate::shaders::common::{iluminar, rampa, tangente_mundo};

pub struct RockyShader {
    pub noise: Noise,
    pub normal_map: Arc<Texture>,
    /// Cuanto se inclina la normal leida del mapa.
    pub normal_strength: f32,
    /// Paradas de color por altura, de la mas baja a la mas alta.
    pub paleta: Vec<(f32, Vec3)>,
    /// Escala del grano fino del albedo.
    pub detalle: f32,
    pub especular: f32,
}

impl FragmentShader for RockyShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let encoded = self.normal_map.sample(frag.uv);
        let n = apply_normal_map_tbn(
            frag.normal,
            tangente_mundo(frag, u),
            encoded,
            self.normal_strength,
        );

        let dir = frag.local.normalized();
        let grano = self.noise.fbm(dir * self.detalle, 4, 2.0, 0.5) * 0.5 + 0.5;

        let h = frag.height.clamp(-1.0, 1.0);
        let mut albedo = rampa(&self.paleta, h * 0.5 + 0.5);
        // El grano oscurece o aclara un poco cada zona.
        albedo = albedo * (0.78 + grano * 0.44);

        // Las laderas muy inclinadas respecto del radio quedan mas
        // oscuras, como roca expuesta.
        let pendiente = 1.0 - n.dot(u.normal_matrix.mul_dir(dir).normalized()).clamp(0.0, 1.0);
        albedo = albedo * (1.0 - smoothstep(0.1, 0.6, pendiente) * 0.35);

        Vec4::from_vec3(iluminar(albedo, n, frag, u, 24.0, self.especular), 1.0)
    }
}
