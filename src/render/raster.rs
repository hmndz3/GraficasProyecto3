//! Rasterizador de triangulos con prueba de profundidad.
//!
//! El recorrido es el clasico: recorte contra el plano cercano, division
//! perspectiva, paso a coordenadas de pantalla, descarte de caras traseras
//! y relleno por coordenadas baricentricas con correccion perspectiva.
//!
//! El framebuffer se parte en bandas horizontales y cada hilo escribe solo
//! en las filas que le tocan, asi que no hay bloqueos ni pixeles en
//! disputa.

use super::framebuffer::{pack_color, Band, Framebuffer};
use super::pipeline::{
    run_vertex_stage, Blend, ClipVertex, CullMode, DrawOptions, Fragment, FragmentShader,
    RasterStats, Uniforms,
};
use crate::geometry::mesh::Mesh;
use crate::math::{Vec2, Vec3};
use rayon::prelude::*;
use std::sync::atomic::AtomicU64;

/// Margen del plano cercano en espacio de recorte.
const CLIP_EPS: f32 = 1e-5;

/// Vertice ya proyectado a pantalla.
#[derive(Debug, Clone, Copy)]
struct ScreenVertex {
    x: f32,
    y: f32,
    /// Profundidad en espacio normalizado, lineal en pantalla.
    z: f32,
    /// 1/w, necesario para la interpolacion con correccion perspectiva.
    inv_w: f32,
    world: Vec3,
    local: Vec3,
    normal: Vec3,
    uv: Vec2,
    height: f32,
}

/// Recorta un triangulo contra el plano cercano `z + w > 0`.
///
/// Devuelve de cero a dos triangulos. Sin este paso, un vertice detras de
/// la camara produce una division por un `w` negativo y el triangulo se
/// dibuja invertido a lo largo de toda la pantalla.
fn clip_near(tri: [ClipVertex; 3], out: &mut Vec<[ClipVertex; 3]>) {
    let dist = |v: &ClipVertex| v.clip.z + v.clip.w;

    let mut poly: Vec<ClipVertex> = Vec::with_capacity(4);
    for i in 0..3 {
        let cur = tri[i];
        let nxt = tri[(i + 1) % 3];
        let dc = dist(&cur);
        let dn = dist(&nxt);

        if dc >= CLIP_EPS {
            poly.push(cur);
        }
        // El borde cruza el plano: se inserta el punto de corte.
        if (dc >= CLIP_EPS) != (dn >= CLIP_EPS) {
            let denom = dc - dn;
            if denom.abs() > 1e-12 {
                let t = (dc - CLIP_EPS) / denom;
                poly.push(ClipVertex::lerp(&cur, &nxt, t.clamp(0.0, 1.0)));
            }
        }
    }

    // Abanico de triangulos sobre el poligono resultante.
    for i in 1..poly.len().saturating_sub(1) {
        out.push([poly[0], poly[i], poly[i + 1]]);
    }
}

#[inline]
fn to_screen(v: &ClipVertex, width: f32, height: f32) -> ScreenVertex {
    let inv_w = 1.0 / v.clip.w;
    let ndc_x = v.clip.x * inv_w;
    let ndc_y = v.clip.y * inv_w;
    let ndc_z = v.clip.z * inv_w;
    ScreenVertex {
        x: (ndc_x * 0.5 + 0.5) * width,
        // La y de pantalla crece hacia abajo, al reves que la de NDC.
        y: (0.5 - ndc_y * 0.5) * height,
        z: ndc_z,
        inv_w,
        world: v.world,
        local: v.local,
        normal: v.normal,
        uv: v.uv,
        height: v.height,
    }
}

#[inline(always)]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Dibuja una malla completa. Es el unico punto de entrada que usa la
/// escena.
///
/// El trabajo se hace en dos fases. Primero se ensamblan los triangulos:
/// etapa de vertices, recorte contra el plano cercano, division
/// perspectiva, paso a pantalla y descarte de caras traseras y de
/// triangulos fuera de cuadro. Esa fase ocurre una sola vez por triangulo
/// y es la que alimenta los contadores. Despues se rellenan los pixeles,
/// repartiendo las filas del framebuffer entre los hilos.
pub fn draw_mesh(
    fb: &mut Framebuffer,
    mesh: &Mesh,
    shader: &dyn FragmentShader,
    uniforms: &Uniforms,
    options: DrawOptions,
    stats: &RasterStats,
) {
    if mesh.indices.is_empty() {
        return;
    }
    RasterStats::add(&stats.meshes_submitted, 1);
    RasterStats::add(&stats.triangles_submitted, mesh.indices.len() as u64);

    let verts = run_vertex_stage(mesh, uniforms);
    let width = fb.width as f32;
    let height = fb.height as f32;

    let triangles: Vec<Triangle> = mesh
        .indices
        .par_iter()
        .fold(Vec::new, |mut acc: Vec<Triangle>, tri| {
            let v = [
                verts[tri[0] as usize],
                verts[tri[1] as usize],
                verts[tri[2] as usize],
            ];

            let mut clipped: Vec<[ClipVertex; 3]> = Vec::with_capacity(2);
            // Camino rapido: si los tres vertices estan delante del plano
            // cercano no hace falta recortar.
            if v[0].clip.z + v[0].clip.w >= CLIP_EPS
                && v[1].clip.z + v[1].clip.w >= CLIP_EPS
                && v[2].clip.z + v[2].clip.w >= CLIP_EPS
            {
                clipped.push(v);
            } else {
                clip_near(v, &mut clipped);
            }

            for t in clipped {
                let s = [
                    to_screen(&t[0], width, height),
                    to_screen(&t[1], width, height),
                    to_screen(&t[2], width, height),
                ];
                if let Some(tri) = assemble(s, options, width, height, stats) {
                    acc.push(tri);
                }
            }
            acc
        })
        .reduce(Vec::new, |mut a, mut b| {
            a.append(&mut b);
            a
        });

    if triangles.is_empty() {
        return;
    }

    let threads = rayon::current_num_threads();
    // Varias bandas por hilo reparten mejor el trabajo cuando un planeta
    // cubre solo una franja de la pantalla.
    let band_count = (threads * 4).max(1);

    fb.bands(band_count).into_par_iter().for_each(|mut band| {
        for tri in &triangles {
            if tri.max_y <= band.y0 as i64 || tri.min_y >= band.y1 as i64 {
                continue;
            }
            raster_triangle(&mut band, tri, shader, uniforms, options, stats);
        }
    });
}

/// Triangulo listo para rellenar: ya proyectado, con su rectangulo
/// envolvente recortado a la pantalla.
struct Triangle {
    v: [ScreenVertex; 3],
    inv_area: f32,
    min_x: i64,
    max_x: i64,
    min_y: i64,
    max_y: i64,
}

/// Descarta el triangulo o lo prepara para el relleno.
fn assemble(
    s: [ScreenVertex; 3],
    options: DrawOptions,
    width: f32,
    height: f32,
    stats: &RasterStats,
) -> Option<Triangle> {
    let area = edge(s[0].x, s[0].y, s[1].x, s[1].y, s[2].x, s[2].y);

    // Tras invertir la y de pantalla, las caras de frente quedan con area
    // negativa.
    match options.cull {
        CullMode::Back => {
            if area >= -1e-7 {
                RasterStats::add(&stats.triangles_backface, 1);
                return None;
            }
        }
        CullMode::None => {
            if area.abs() < 1e-7 {
                RasterStats::add(&stats.triangles_backface, 1);
                return None;
            }
        }
    }

    let min_x = s[0].x.min(s[1].x).min(s[2].x).floor().max(0.0) as i64;
    let max_x = s[0].x.max(s[1].x).max(s[2].x).ceil().min(width) as i64;
    let min_y = s[0].y.min(s[1].y).min(s[2].y).floor().max(0.0) as i64;
    let max_y = s[0].y.max(s[1].y).max(s[2].y).ceil().min(height) as i64;

    if min_x >= max_x || min_y >= max_y {
        RasterStats::add(&stats.triangles_offscreen, 1);
        return None;
    }

    RasterStats::add(&stats.triangles_rasterized, 1);
    Some(Triangle { v: s, inv_area: 1.0 / area, min_x, max_x, min_y, max_y })
}

fn raster_triangle(
    band: &mut Band<'_>,
    tri: &Triangle,
    shader: &dyn FragmentShader,
    uniforms: &Uniforms,
    options: DrawOptions,
    stats: &RasterStats,
) {
    let s0 = &tri.v[0];
    let s1 = &tri.v[1];
    let s2 = &tri.v[2];

    let min_x = tri.min_x;
    let max_x = tri.max_x;
    let min_y = tri.min_y.max(band.y0 as i64);
    let max_y = tri.max_y.min(band.y1 as i64);
    if min_y >= max_y {
        return;
    }

    let inv_area = tri.inv_area;

    // Funciones de borde evaluadas de forma incremental: se calcula el
    // valor en la esquina del rectangulo y despues solo se suman pasos.
    let (a0, b0) = (-(s2.y - s1.y), s2.x - s1.x);
    let (a1, b1) = (-(s0.y - s2.y), s0.x - s2.x);
    let (a2, b2) = (-(s1.y - s0.y), s1.x - s0.x);

    let px = min_x as f32 + 0.5;
    let py = min_y as f32 + 0.5;
    let mut row0 = edge(s1.x, s1.y, s2.x, s2.y, px, py);
    let mut row1 = edge(s2.x, s2.y, s0.x, s0.y, px, py);
    let mut row2 = edge(s0.x, s0.y, s1.x, s1.y, px, py);

    let mut shaded: u64 = 0;

    for y in min_y..max_y {
        let mut e0 = row0;
        let mut e1 = row1;
        let mut e2 = row2;

        for x in min_x..max_x {
            let w0 = e0 * inv_area;
            let w1 = e1 * inv_area;
            let w2 = e2 * inv_area;

            e0 += a0;
            e1 += a1;
            e2 += a2;

            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }

            let depth = w0 * s0.z + w1 * s1.z + w2 * s2.z;
            if !(-1.0..=1.0).contains(&depth) {
                continue;
            }

            let idx = band.index(x as usize, y as usize);
            if depth >= band.depth[idx] {
                continue;
            }

            // Correccion perspectiva: los atributos se interpolan
            // divididos entre w y al final se recupera el valor real.
            let iw = w0 * s0.inv_w + w1 * s1.inv_w + w2 * s2.inv_w;
            if iw.abs() < 1e-20 {
                continue;
            }
            let inv_iw = 1.0 / iw;
            let c0 = w0 * s0.inv_w * inv_iw;
            let c1 = w1 * s1.inv_w * inv_iw;
            let c2 = w2 * s2.inv_w * inv_iw;

            let frag = Fragment {
                world: s0.world * c0 + s1.world * c1 + s2.world * c2,
                local: s0.local * c0 + s1.local * c1 + s2.local * c2,
                normal: (s0.normal * c0 + s1.normal * c1 + s2.normal * c2).normalized(),
                uv: Vec2::new(
                    s0.uv.x * c0 + s1.uv.x * c1 + s2.uv.x * c2,
                    s0.uv.y * c0 + s1.uv.y * c1 + s2.uv.y * c2,
                ),
                height: s0.height * c0 + s1.height * c1 + s2.height * c2,
                depth,
            };

            let out = shader.shade(&frag, uniforms);
            shaded += 1;

            match options.blend {
                Blend::Opaque => {
                    band.depth[idx] = depth;
                    band.color[idx] = pack_color(out.xyz());
                }
                Blend::Additive => {
                    let alpha = out.w.clamp(0.0, 1.0);
                    if alpha > 0.001 {
                        let dst = unpack_color(band.color[idx]);
                        band.color[idx] = pack_color(dst + out.xyz() * alpha);
                    }
                }
            }
        }

        row0 += b0;
        row1 += b1;
        row2 += b2;
    }

    if shaded > 0 {
        RasterStats::add(&stats.fragments_shaded, shaded);
    }
}

/// Deshace `pack_color` de forma aproximada para poder sumar color encima.
/// Solo lo usa la mezcla aditiva, que no necesita precision exacta.
#[inline]
fn unpack_color(c: u32) -> Vec3 {
    let r = ((c >> 16) & 0xFF) as f32 / 255.0;
    let g = ((c >> 8) & 0xFF) as f32 / 255.0;
    let b = (c & 0xFF) as f32 / 255.0;
    let lin = Vec3::new(r.powf(2.2), g.powf(2.2), b.powf(2.2));
    // Inversa del mapeo de tono de Reinhard.
    Vec3::new(
        lin.x / (1.0 - lin.x).max(1e-3),
        lin.y / (1.0 - lin.y).max(1e-3),
        lin.z / (1.0 - lin.z).max(1e-3),
    )
}

/// Recorre todos los pixeles en paralelo. Lo usa el fondo estrellado, que
/// se resuelve por pixel y no por triangulo.
pub fn for_each_pixel<F>(fb: &mut Framebuffer, f: F)
where
    F: Fn(usize, usize) -> Vec3 + Sync,
{
    let threads = rayon::current_num_threads().max(1);
    fb.bands(threads * 2).into_par_iter().for_each(|band| {
        for y in band.y0..band.y1 {
            for x in 0..band.width {
                let idx = (y - band.y0) * band.width + x;
                band.color[idx] = pack_color(f(x, y));
            }
        }
    });
}

/// Dibuja un punto cuadrado con mezcla aditiva y sin prueba de
/// profundidad. Es como se pintan las estrellas del fondo.
pub fn splat_point(fb: &mut Framebuffer, x: i64, y: i64, radius: i64, color: Vec3) {
    for dy in -radius..=radius {
        let py = y + dy;
        if py < 0 || py >= fb.height as i64 {
            continue;
        }
        for dx in -radius..=radius {
            let px = x + dx;
            if px < 0 || px >= fb.width as i64 {
                continue;
            }
            let idx = py as usize * fb.width + px as usize;
            let dst = unpack_color(fb.color[idx]);
            fb.color[idx] = pack_color(dst + color);
        }
    }
}

/// Contador auxiliar para medir trabajo fuera del rasterizador.
pub fn bump(counter: &AtomicU64, n: u64) {
    RasterStats::add(counter, n);
}
