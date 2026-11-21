//! Piezas que comparten los fragment shaders: modelo de iluminacion,
//! utilidades de color y la base tangente de los planetas.

use crate::math::{smoothstep, Vec3};
use crate::render::pipeline::{Fragment, Uniforms};

/// Luz ambiental minima para que la cara nocturna no quede negra pura.
pub const AMBIENTE: Vec3 = Vec3 { x: 0.035, y: 0.040, z: 0.058 };

/// Direccion desde el fragmento hacia la estrella.
#[inline]
pub fn direccion_luz(frag: &Fragment, u: &Uniforms) -> Vec3 {
    (u.light_world - frag.world).normalized()
}

/// Direccion desde el fragmento hacia la camara.
#[inline]
pub fn direccion_vista(frag: &Fragment, u: &Uniforms) -> Vec3 {
    (u.camera_world - frag.world).normalized()
}

/// Termino difuso de Lambert con un borde suavizado.
///
/// El suavizado evita el corte duro del terminador, que en una esfera de
/// pocos triangulos se nota como un filo poligonal.
#[inline]
pub fn difuso(n: Vec3, l: Vec3) -> f32 {
    let d = n.dot(l);
    smoothstep(-0.08, 0.22, d) * d.max(0.0).powf(0.85)
}

/// Reflejo especular de Blinn-Phong.
#[inline]
pub fn especular(n: Vec3, l: Vec3, v: Vec3, brillo: f32) -> f32 {
    if n.dot(l) <= 0.0 {
        return 0.0;
    }
    let h = (l + v).normalized();
    n.dot(h).max(0.0).powf(brillo)
}

/// Realce de borde: vale casi uno donde la superficie se aleja de la
/// camara. Sirve para atmosferas y para despegar la silueta del fondo.
#[inline]
pub fn borde(n: Vec3, v: Vec3, potencia: f32) -> f32 {
    (1.0 - n.dot(v).abs()).clamp(0.0, 1.0).powf(potencia)
}

/// Mezcla lineal entre dos colores.
#[inline]
pub fn mezcla(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a.lerp(b, t.clamp(0.0, 1.0))
}

/// Rampa de color de cuatro paradas, usada para pintar el terreno segun la
/// altura.
pub fn rampa(paradas: &[(f32, Vec3)], t: f32) -> Vec3 {
    if paradas.is_empty() {
        return Vec3::ZERO;
    }
    if t <= paradas[0].0 {
        return paradas[0].1;
    }
    for par in paradas.windows(2) {
        let (t0, c0) = par[0];
        let (t1, c1) = par[1];
        if t <= t1 {
            let k = if (t1 - t0).abs() < 1e-6 { 0.0 } else { (t - t0) / (t1 - t0) };
            return c0.lerp(c1, k);
        }
    }
    paradas[paradas.len() - 1].1
}

/// Tangente en espacio de objeto para la parametrizacion equirectangular
/// de una esfera, llevada a espacio de mundo.
///
/// Es lo que permite aplicar el mapa de normales sobre un planeta que
/// gira: la base acompaña al modelo en lugar de quedarse fija al mundo.
#[inline]
pub fn tangente_mundo(frag: &Fragment, u: &Uniforms) -> Vec3 {
    let dir = frag.local.normalized();
    let mut t = Vec3::UP.cross(dir);
    if t.length_squared() < 1e-8 {
        t = Vec3::new(1.0, 0.0, 0.0);
    }
    u.normal_matrix.mul_dir(t.normalized())
}

/// Ilumina un color base con la estrella y devuelve el color final.
pub fn iluminar(
    albedo: Vec3,
    normal: Vec3,
    frag: &Fragment,
    u: &Uniforms,
    brillo_especular: f32,
    fuerza_especular: f32,
) -> Vec3 {
    let l = direccion_luz(frag, u);
    let v = direccion_vista(frag, u);
    let d = difuso(normal, l);
    let s = especular(normal, l, v, brillo_especular) * fuerza_especular;
    albedo.mul_elem(AMBIENTE + Vec3::splat(d)) + Vec3::splat(s)
}
