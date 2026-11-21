//! Matrices 4x4 y cuaterniones.
//!
//! Las matrices se guardan por filas: `m[fila][columna]`. Un punto se
//! transforma como `M * v` (vector columna), que es la convencion que usa
//! el pipeline en `render::pipeline`.

use super::vector::{Vec3, Vec4};
use std::ops::Mul;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::identity()
    }
}

impl Mat4 {
    pub const fn from_rows(m: [[f32; 4]; 4]) -> Self {
        Self { m }
    }

    pub const fn identity() -> Self {
        Self::from_rows([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    pub const fn zero() -> Self {
        Self::from_rows([[0.0; 4]; 4])
    }

    pub fn translation(t: Vec3) -> Self {
        let mut r = Self::identity();
        r.m[0][3] = t.x;
        r.m[1][3] = t.y;
        r.m[2][3] = t.z;
        r
    }

    pub fn scale(s: Vec3) -> Self {
        let mut r = Self::identity();
        r.m[0][0] = s.x;
        r.m[1][1] = s.y;
        r.m[2][2] = s.z;
        r
    }

    pub fn uniform_scale(s: f32) -> Self {
        Self::scale(Vec3::splat(s))
    }

    pub fn rotation_x(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self::from_rows([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, c, -s, 0.0],
            [0.0, s, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    pub fn rotation_y(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self::from_rows([
            [c, 0.0, s, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [-s, 0.0, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    pub fn rotation_z(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self::from_rows([
            [c, -s, 0.0, 0.0],
            [s, c, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Rotacion de `angle` radianes alrededor de un eje arbitrario
    /// (formula de Rodrigues en forma matricial). La usan la inclinacion
    /// axial de los planetas y la orientacion de los objetos de superficie.
    pub fn rotation_axis(axis: Vec3, angle: f32) -> Self {
        let a = axis.normalized();
        if a == Vec3::ZERO {
            return Self::identity();
        }
        let (s, c) = angle.sin_cos();
        let t = 1.0 - c;
        let (x, y, z) = (a.x, a.y, a.z);
        Self::from_rows([
            [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.0],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.0],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Construye una base ortonormal a partir de tres ejes y una posicion.
    /// Es la forma barata de armar la matriz de modelo de la nave o de un
    /// arbol parado sobre la superficie curva de un planeta.
    pub fn from_basis(right: Vec3, up: Vec3, forward: Vec3, origin: Vec3) -> Self {
        Self::from_rows([
            [right.x, up.x, forward.x, origin.x],
            [right.y, up.y, forward.y, origin.y],
            [right.z, up.z, forward.z, origin.z],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Matriz de vista para una camara en `eye` mirando hacia `target`.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalized();
        let mut r = f.cross(up).normalized();
        if r == Vec3::ZERO {
            // El objetivo quedo alineado con `up`: se escoge otro eje para
            // que la base no colapse.
            r = f.cross(Vec3::new(0.0, 0.0, 1.0)).normalized();
        }
        let u = r.cross(f);
        Self::from_rows([
            [r.x, r.y, r.z, -r.dot(eye)],
            [u.x, u.y, u.z, -u.dot(eye)],
            [-f.x, -f.y, -f.z, f.dot(eye)],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Proyeccion en perspectiva con rango de profundidad [-1, 1].
    /// `fov_y` va en radianes.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let mut r = Self::zero();
        r.m[0][0] = f / aspect;
        r.m[1][1] = f;
        r.m[2][2] = (far + near) / (near - far);
        r.m[2][3] = (2.0 * far * near) / (near - far);
        r.m[3][2] = -1.0;
        r
    }

    pub fn transpose(&self) -> Self {
        let mut r = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                r.m[i][j] = self.m[j][i];
            }
        }
        r
    }

    pub fn mul_vec4(&self, v: Vec4) -> Vec4 {
        Vec4::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z + self.m[0][3] * v.w,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z + self.m[1][3] * v.w,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z + self.m[2][3] * v.w,
            self.m[3][0] * v.x + self.m[3][1] * v.y + self.m[3][2] * v.z + self.m[3][3] * v.w,
        )
    }

    /// Transforma un punto (w = 1) sin dividir por w.
    pub fn mul_point(&self, p: Vec3) -> Vec3 {
        self.mul_vec4(Vec4::from_vec3(p, 1.0)).xyz()
    }

    /// Transforma una direccion (w = 0): ignora la traslacion.
    pub fn mul_dir(&self, d: Vec3) -> Vec3 {
        self.mul_vec4(Vec4::from_vec3(d, 0.0)).xyz()
    }

    /// Inversa de una matriz rigida con escala uniforme (rotacion +
    /// traslacion + escala). Es todo lo que necesita la escena y evita
    /// calcular una inversa general de 4x4 en cada cuadro.
    pub fn inverse_rigid(&self) -> Self {
        let sx = Vec3::new(self.m[0][0], self.m[1][0], self.m[2][0]).length();
        let inv_s2 = if sx <= 1e-12 { 1.0 } else { 1.0 / (sx * sx) };
        let mut r = Self::identity();
        for i in 0..3 {
            for j in 0..3 {
                r.m[i][j] = self.m[j][i] * inv_s2;
            }
        }
        let t = Vec3::new(self.m[0][3], self.m[1][3], self.m[2][3]);
        let it = Vec3::new(
            r.m[0][0] * t.x + r.m[0][1] * t.y + r.m[0][2] * t.z,
            r.m[1][0] * t.x + r.m[1][1] * t.y + r.m[1][2] * t.z,
            r.m[2][0] * t.x + r.m[2][1] * t.y + r.m[2][2] * t.z,
        );
        r.m[0][3] = -it.x;
        r.m[1][3] = -it.y;
        r.m[2][3] = -it.z;
        r
    }

    /// Matriz para transformar normales: la traspuesta de la inversa de la
    /// parte 3x3. Con escala no uniforme (los anillos, por ejemplo) la
    /// normal se deforma si se usa la matriz de modelo tal cual.
    pub fn normal_matrix(&self) -> Self {
        let a = &self.m;
        let (a00, a01, a02) = (a[0][0], a[0][1], a[0][2]);
        let (a10, a11, a12) = (a[1][0], a[1][1], a[1][2]);
        let (a20, a21, a22) = (a[2][0], a[2][1], a[2][2]);

        let c00 = a11 * a22 - a12 * a21;
        let c01 = a12 * a20 - a10 * a22;
        let c02 = a10 * a21 - a11 * a20;
        let det = a00 * c00 + a01 * c01 + a02 * c02;
        if det.abs() <= 1e-12 {
            return Self::identity();
        }
        let inv_det = 1.0 / det;

        // cofactores / det ya equivale a la traspuesta de la inversa
        let mut r = Self::identity();
        r.m[0][0] = c00 * inv_det;
        r.m[0][1] = c01 * inv_det;
        r.m[0][2] = c02 * inv_det;
        r.m[1][0] = (a02 * a21 - a01 * a22) * inv_det;
        r.m[1][1] = (a00 * a22 - a02 * a20) * inv_det;
        r.m[1][2] = (a01 * a20 - a00 * a21) * inv_det;
        r.m[2][0] = (a01 * a12 - a02 * a11) * inv_det;
        r.m[2][1] = (a02 * a10 - a00 * a12) * inv_det;
        r.m[2][2] = (a00 * a11 - a01 * a10) * inv_det;
        r
    }
}

impl Mul for Mat4 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut r = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                let mut acc = 0.0;
                for k in 0..4 {
                    acc += self.m[i][k] * o.m[k][j];
                }
                r.m[i][j] = acc;
            }
        }
        r
    }
}
