//! Shader de planeta con oceano, continentes y nubes animadas.
//!
//! El nivel del mar separa dos materiales distintos: el agua usa la normal
//! geometrica, casi plana, con un especular fuerte y angosto; la tierra
//! usa el mapa de normales y un especular apagado. Encima se mezcla una
//! capa de nubes que se desplaza con el tiempo.

use std::sync::Arc;

use crate::math::{smoothstep, Vec3, Vec4};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::render::texture::{apply_normal_map_tbn, Texture};
use crate::shaders::common::{
    borde, difuso, direccion_luz, direccion_vista, especular, iluminar, mezcla, rampa,
    tangente_mundo, AMBIENTE,
};

pub struct OceanShader {
    pub noise: Noise,
    pub normal_map: Arc<Texture>,
    pub normal_strength: f32,
    /// Altura, en el rango del terreno, donde empieza la tierra firme.
    pub nivel_mar: f32,
    pub agua_profunda: Vec3,
    pub agua_baja: Vec3,
    pub paleta_tierra: Vec<(f32, Vec3)>,
    /// Cobertura de nubes: 0 despejado, 1 cubierto.
    pub nubes: f32,
    pub velocidad_nubes: f32,
    pub color_atmosfera: Vec3,
}

impl FragmentShader for OceanShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let dir = frag.local.normalized();
        let h = frag.height.clamp(-1.0, 1.0);
        let l = direccion_luz(frag, u);
        let v = direccion_vista(frag, u);

        let es_tierra = smoothstep(self.nivel_mar - 0.015, self.nivel_mar + 0.015, h);

        let color = if es_tierra < 0.999 {
            // Agua: el color se oscurece con la profundidad y el reflejo
            // del sol es muy concentrado.
            let prof = ((self.nivel_mar - h) / 0.35).clamp(0.0, 1.0);
            let agua = mezcla(self.agua_baja, self.agua_profunda, prof);
            let n_agua = frag.normal.normalized();
            // Pequeño oleaje para que el reflejo no sea un disco perfecto.
            let olas = self.noise.fbm(
                dir * 42.0 + Vec3::splat(u.time * 0.35),
                3,
                2.0,
                0.5,
            ) * 0.035;
            let n_agua = (n_agua + Vec3::splat(olas)).normalized();
            let d = difuso(n_agua, l);
            let s = especular(n_agua, l, v, 420.0) * 1.1;
            let agua_lit = agua.mul_elem(AMBIENTE + Vec3::splat(d)) + Vec3::splat(s);

            if es_tierra <= 0.001 {
                agua_lit
            } else {
                let tierra = self.tierra(frag, u, h);
                mezcla(agua_lit, tierra, es_tierra)
            }
        } else {
            self.tierra(frag, u, h)
        };

        // Nubes: dos capas de ruido que se mueven a distinta velocidad.
        let t = u.time * self.velocidad_nubes;
        let c1 = self
            .noise
            .fbm(dir * 3.1 + Vec3::new(t, t * 0.3, -t * 0.5), 5, 2.1, 0.55);
        let c2 = self
            .noise
            .fbm(dir * 6.3 + Vec3::new(-t * 0.7, 0.0, t * 0.9), 4, 2.0, 0.5);
        let densidad = smoothstep(0.52 - self.nubes * 0.5, 0.78, (c1 * 0.65 + c2 * 0.35) * 0.5 + 0.5);
        let luz_nube = difuso(frag.normal, l);
        let color = mezcla(
            color,
            Vec3::splat(0.76).mul_elem(AMBIENTE + Vec3::splat(luz_nube)),
            densidad * self.nubes,
        );

        // Halo atmosferico en el borde, mas intenso del lado iluminado.
        let halo = borde(frag.normal, v, 3.0) * difuso(frag.normal, l).max(0.08);
        let color = color + self.color_atmosfera * halo * 0.60;

        Vec4::from_vec3(color, 1.0)
    }
}

impl OceanShader {
    fn tierra(&self, frag: &Fragment, u: &Uniforms, h: f32) -> Vec3 {
        let encoded = self.normal_map.sample(frag.uv);
        let n = apply_normal_map_tbn(
            frag.normal,
            tangente_mundo(frag, u),
            encoded,
            self.normal_strength,
        );
        // La rampa se normaliza respecto del nivel del mar para que la
        // costa siempre caiga en el primer color.
        let t = ((h - self.nivel_mar) / (1.0 - self.nivel_mar)).clamp(0.0, 1.0);
        let grano = self.noise.fbm(frag.local.normalized() * 18.0, 4, 2.0, 0.5) * 0.5 + 0.5;
        let albedo = rampa(&self.paleta_tierra, t) * (0.82 + grano * 0.36);
        iluminar(albedo, n, frag, u, 18.0, 0.06)
    }
}
