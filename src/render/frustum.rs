//! Descarte por volumen de vision.
//!
//! Antes de transformar un solo vertice se comprueba si la esfera
//! envolvente del objeto cae dentro de la piramide de vision. Los seis
//! planos se extraen directamente de la matriz vista-proyeccion.

use crate::math::{Mat4, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    /// Distancia con signo de un punto al plano. Positiva del lado
    /// interior.
    #[inline]
    pub fn distance(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Extrae los planos de la matriz vista-proyeccion.
    ///
    /// Un punto esta dentro si cumple `-w <= x,y,z <= w`. Cada una de esas
    /// seis desigualdades es una combinacion lineal de las filas de la
    /// matriz, y esa combinacion es justamente el plano.
    pub fn from_view_projection(vp: &Mat4) -> Self {
        let m = &vp.m;
        let mut planes = [Plane { normal: Vec3::ZERO, d: 0.0 }; 6];

        let filas = [
            // izquierda, derecha
            (3, 0, 1.0f32),
            (3, 0, -1.0),
            // abajo, arriba
            (3, 1, 1.0),
            (3, 1, -1.0),
            // cercano, lejano
            (3, 2, 1.0),
            (3, 2, -1.0),
        ];

        for (i, (fw, fc, signo)) in filas.iter().enumerate() {
            let n = Vec3::new(
                m[*fw][0] + signo * m[*fc][0],
                m[*fw][1] + signo * m[*fc][1],
                m[*fw][2] + signo * m[*fc][2],
            );
            let d = m[*fw][3] + signo * m[*fc][3];
            // Se normaliza para que la distancia tenga unidades de mundo.
            let len = n.length();
            if len > 1e-12 {
                planes[i] = Plane { normal: n / len, d: d / len };
            } else {
                planes[i] = Plane { normal: Vec3::new(0.0, 0.0, 1.0), d: 0.0 };
            }
        }

        Self { planes }
    }

    /// Verdadero si la esfera toca o esta dentro del volumen de vision.
    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p.distance(center) >= -radius)
    }
}
