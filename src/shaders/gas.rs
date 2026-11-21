//! Shader de gigante gaseoso: bandas con rotacion diferencial y una
//! tormenta persistente.
//!
//! El patron no se dibuja sobre la superficie quieta: cada latitud avanza
//! a distinta velocidad, igual que en un planeta gaseoso real, y eso hace
//! que las bandas se corten y se mezclen entre si con el tiempo.

use crate::math::{smoothstep, Vec3, Vec4, PI};
use crate::noise::Noise;
use crate::render::pipeline::{Fragment, FragmentShader, Uniforms};
use crate::shaders::common::{borde, difuso, direccion_luz, direccion_vista, iluminar, mezcla};

pub struct GasShader {
    pub noise: Noise,
    /// Cuantas bandas caben del polo al polo.
    pub bandas: f32,
    /// Velocidad base de la rotacion diferencial.
    pub deriva: f32,
    /// Cuanta turbulencia deforma el borde de cada banda.
    pub turbulencia: f32,
    pub color_claro: Vec3,
    pub color_oscuro: Vec3,
    pub color_tormenta: Vec3,
    /// Latitud del centro de la tormenta, en el rango [-1, 1].
    pub tormenta_lat: f32,
    /// Longitud del centro de la tormenta, en radianes.
    pub tormenta_lon: f32,
    pub tormenta_tam: f32,
    pub color_atmosfera: Vec3,
}

impl FragmentShader for GasShader {
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4 {
        let dir = frag.local.normalized();
        let lat = dir.y.clamp(-1.0, 1.0);
        let lon = dir.z.atan2(dir.x);

        // Rotacion diferencial: el ecuador corre mas rapido que los polos.
        let velocidad = self.deriva * (1.0 - lat * lat * 0.75);
        let lon_desplazada = lon + u.time * velocidad;

        // Punto de muestreo con la longitud ya corrida.
        let r = (1.0 - lat * lat).max(0.0).sqrt();
        let p = Vec3::new(
            r * lon_desplazada.cos(),
            lat,
            r * lon_desplazada.sin(),
        );

        // Turbulencia estirada en horizontal: el ruido se comprime en y
        // para que los remolinos salgan alargados como en las bandas.
        let turb = self
            .noise
            .fbm(Vec3::new(p.x * 2.0, p.y * 7.0, p.z * 2.0), 5, 2.1, 0.55)
            * self.turbulencia;

        let onda = ((lat * self.bandas + turb) * PI).sin();
        let t = onda * 0.5 + 0.5;
        let mut albedo = mezcla(self.color_oscuro, self.color_claro, t);

        // Vetas finas encima de las bandas.
        let veta = self
            .noise
            .fbm(Vec3::new(p.x * 5.0, p.y * 26.0, p.z * 5.0), 3, 2.0, 0.5);
        albedo = albedo * (0.9 + veta * 0.2);

        // Tormenta: un ovalo que tambien se arrastra con la deriva de su
        // latitud.
        let centro_lon = self.tormenta_lon + u.time * self.deriva * (1.0 - self.tormenta_lat.powi(2) * 0.75);
        let mut d_lon = lon - centro_lon;
        while d_lon > PI {
            d_lon -= 2.0 * PI;
        }
        while d_lon < -PI {
            d_lon += 2.0 * PI;
        }
        let d_lat = lat - self.tormenta_lat;
        // El ovalo es mas ancho que alto.
        let dist = ((d_lon / 1.9).powi(2) + (d_lat / 0.72).powi(2)).sqrt() / self.tormenta_tam;
        let remolino = self.noise.fbm(p * 9.0, 3, 2.0, 0.5) * 0.25;
        let mascara = 1.0 - smoothstep(0.55, 1.0, dist + remolino);
        albedo = mezcla(albedo, self.color_tormenta, mascara);

        let color = iluminar(albedo, frag.normal, frag, u, 10.0, 0.02);

        // Neblina en el borde del disco.
        let l = direccion_luz(frag, u);
        let v = direccion_vista(frag, u);
        let halo = borde(frag.normal, v, 2.6) * difuso(frag.normal, l).max(0.06);
        Vec4::from_vec3(color + self.color_atmosfera * halo * 0.9, 1.0)
    }
}
