//! Etapas del pipeline por software: vertex stage, recorte, division
//! perspectiva, transformacion a pantalla y llamada al fragment shader.

use crate::geometry::mesh::Mesh;
use crate::math::{Mat4, Vec2, Vec3, Vec4};
use std::sync::atomic::{AtomicU64, Ordering};

/// Datos constantes durante una llamada de dibujo. Es el equivalente a un
/// uniform buffer, pero resuelto en CPU.
#[derive(Debug, Clone, Copy)]
pub struct Uniforms {
    pub model: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub normal_matrix: Mat4,
    /// Posicion de la estrella en espacio de mundo: la unica fuente de luz.
    pub light_world: Vec3,
    pub camera_world: Vec3,
    /// Segundos simulados desde el arranque. Los shaders animados dependen
    /// de este valor y no del numero de cuadros dibujados.
    pub time: f32,
    /// Escala uniforme del objeto, para que el ruido 3D tenga la misma
    /// frecuencia aparente en cuerpos de distinto tamaño.
    pub object_scale: f32,
    /// Tinte por instancia. Permite que un mismo shader pinte muchas
    /// copias del mismo modelo con variaciones de color.
    pub tint: Vec3,
    /// Parametro libre por instancia, entre 0 y 1. Los objetos de
    /// superficie lo usan como semilla de su variacion.
    pub param: f32,
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            model: Mat4::identity(),
            view: Mat4::identity(),
            projection: Mat4::identity(),
            normal_matrix: Mat4::identity(),
            light_world: Vec3::ZERO,
            camera_world: Vec3::ZERO,
            time: 0.0,
            object_scale: 1.0,
            tint: Vec3::ONE,
            param: 0.0,
        }
    }
}

/// Lo que recibe el fragment shader en cada pixel cubierto.
#[derive(Debug, Clone, Copy)]
pub struct Fragment {
    /// Posicion interpolada en espacio de mundo.
    pub world: Vec3,
    /// Posicion interpolada en espacio de objeto. Los shaders evaluan el
    /// ruido aqui para que el patron gire junto con el planeta.
    pub local: Vec3,
    /// Normal interpolada y normalizada, en espacio de mundo.
    pub normal: Vec3,
    pub uv: Vec2,
    pub height: f32,
    /// Profundidad en espacio normalizado, util para efectos de distancia.
    pub depth: f32,
}

/// Un fragment shader propio. `Sync` porque el rasterizador reparte las
/// filas del framebuffer entre varios hilos.
pub trait FragmentShader: Sync {
    /// Devuelve color lineal en `xyz` y cobertura en `w`.
    fn shade(&self, frag: &Fragment, u: &Uniforms) -> Vec4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    Back,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    /// Reemplaza el color y escribe profundidad.
    Opaque,
    /// Suma el color sobre lo que ya habia; prueba de profundidad sin
    /// escritura. Lo usan la corona del sol, las atmosferas y el motor.
    Additive,
}

#[derive(Debug, Clone, Copy)]
pub struct DrawOptions {
    pub cull: CullMode,
    pub blend: Blend,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self { cull: CullMode::Back, blend: Blend::Opaque }
    }
}

impl DrawOptions {
    pub fn opaque() -> Self {
        Self::default()
    }

    pub fn double_sided() -> Self {
        Self { cull: CullMode::None, blend: Blend::Opaque }
    }

    pub fn additive() -> Self {
        Self { cull: CullMode::None, blend: Blend::Additive }
    }
}

/// Vertice ya procesado por la etapa de vertices. Viaja en espacio de
/// recorte junto con los atributos que hay que interpolar.
#[derive(Debug, Clone, Copy)]
pub struct ClipVertex {
    pub clip: Vec4,
    pub world: Vec3,
    pub local: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub height: f32,
}

impl ClipVertex {
    pub fn lerp(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
        ClipVertex {
            clip: a.clip + (b.clip - a.clip) * t,
            world: a.world.lerp(b.world, t),
            local: a.local.lerp(b.local, t),
            normal: a.normal.lerp(b.normal, t),
            uv: a.uv + (b.uv - a.uv) * t,
            height: a.height + (b.height - a.height) * t,
        }
    }
}

/// Etapa de vertices: lleva cada vertice de espacio de objeto a espacio de
/// recorte y deja listos los atributos en espacio de mundo.
pub fn run_vertex_stage(mesh: &Mesh, u: &Uniforms) -> Vec<ClipVertex> {
    let mvp = u.projection * u.view * u.model;
    mesh.vertices
        .iter()
        .map(|v| ClipVertex {
            clip: mvp.mul_vec4(Vec4::from_vec3(v.position, 1.0)),
            world: u.model.mul_point(v.position),
            local: v.position,
            normal: u.normal_matrix.mul_dir(v.normal),
            uv: v.uv,
            height: v.height,
        })
        .collect()
}

/// Contadores de la carga real del rasterizador. Se muestran en pantalla
/// para poder comparar el trabajo con y sin descarte o niveles de detalle.
#[derive(Debug, Default)]
pub struct RasterStats {
    pub meshes_submitted: AtomicU64,
    pub meshes_culled: AtomicU64,
    pub triangles_submitted: AtomicU64,
    pub triangles_backface: AtomicU64,
    pub triangles_offscreen: AtomicU64,
    pub triangles_rasterized: AtomicU64,
    pub fragments_shaded: AtomicU64,
}

impl RasterStats {
    pub fn reset(&self) {
        self.meshes_submitted.store(0, Ordering::Relaxed);
        self.meshes_culled.store(0, Ordering::Relaxed);
        self.triangles_submitted.store(0, Ordering::Relaxed);
        self.triangles_backface.store(0, Ordering::Relaxed);
        self.triangles_offscreen.store(0, Ordering::Relaxed);
        self.triangles_rasterized.store(0, Ordering::Relaxed);
        self.fragments_shaded.store(0, Ordering::Relaxed);
    }

    pub fn get(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    pub fn add(counter: &AtomicU64, n: u64) {
        counter.fetch_add(n, Ordering::Relaxed);
    }
}
