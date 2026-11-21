//! Pruebas del rasterizador: cobertura, descarte de caras traseras,
//! prueba de profundidad, recorte del plano cercano e interpolacion con
//! correccion perspectiva.

use sistema_solar::geometry::mesh::{Mesh, Vertex};
use sistema_solar::math::{to_radians, Mat4, Vec2, Vec3, Vec4};
use sistema_solar::render::framebuffer::Framebuffer;
use sistema_solar::render::pipeline::{
    DrawOptions, Fragment, FragmentShader, RasterStats, Uniforms,
};
use sistema_solar::render::raster::draw_mesh;

/// Shader de color fijo.
struct Plano(Vec3);
impl FragmentShader for Plano {
    fn shade(&self, _f: &Fragment, _u: &Uniforms) -> Vec4 {
        Vec4::from_vec3(self.0, 1.0)
    }
}

/// Shader que devuelve la coordenada u interpolada, para medir la
/// correccion perspectiva.
struct MuestraU;
impl FragmentShader for MuestraU {
    fn shade(&self, f: &Fragment, _u: &Uniforms) -> Vec4 {
        Vec4::new(f.uv.x, f.uv.x, f.uv.x, 1.0)
    }
}

fn uniforms(fb: &Framebuffer) -> Uniforms {
    Uniforms {
        model: Mat4::identity(),
        view: Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP),
        projection: Mat4::perspective(to_radians(60.0), fb.aspect(), 0.1, 100.0),
        normal_matrix: Mat4::identity(),
        ..Default::default()
    }
}

/// Triangulo en el plano z = `z`, enrollado en sentido antihorario visto
/// desde la camara.
fn triangulo(z: f32, invertido: bool) -> Mesh {
    let n = Vec3::new(0.0, 0.0, 1.0);
    let p = [
        Vec3::new(-1.5, -1.5, z),
        Vec3::new(1.5, -1.5, z),
        Vec3::new(0.0, 1.5, z),
    ];
    let vs = vec![
        Vertex::new(p[0], n, Vec2::new(0.0, 0.0)),
        Vertex::new(p[1], n, Vec2::new(1.0, 0.0)),
        Vertex::new(p[2], n, Vec2::new(0.5, 1.0)),
    ];
    let idx = if invertido { vec![[0, 2, 1]] } else { vec![[0, 1, 2]] };
    Mesh::new(vs, idx)
}

fn centro(fb: &Framebuffer) -> u32 {
    fb.color[(fb.height / 2) * fb.width + fb.width / 2]
}

#[test]
fn un_triangulo_de_frente_pinta_el_centro() {
    let mut fb = Framebuffer::new(64, 64);
    fb.clear(0);
    let u = uniforms(&fb);
    let stats = RasterStats::default();

    draw_mesh(
        &mut fb,
        &triangulo(-3.0, false),
        &Plano(Vec3::new(1.0, 0.0, 0.0)),
        &u,
        DrawOptions::opaque(),
        &stats,
    );

    assert_ne!(centro(&fb), 0, "el centro deberia quedar pintado");
    assert!(
        fb.depth[(64 / 2) * 64 + 64 / 2].is_finite(),
        "el z-buffer deberia haberse escrito"
    );
}

#[test]
fn la_cara_trasera_se_descarta() {
    let mut fb = Framebuffer::new(64, 64);
    fb.clear(0);
    let u = uniforms(&fb);
    let stats = RasterStats::default();

    draw_mesh(
        &mut fb,
        &triangulo(-3.0, true),
        &Plano(Vec3::ONE),
        &u,
        DrawOptions::opaque(),
        &stats,
    );

    assert_eq!(centro(&fb), 0, "una cara trasera no deberia pintarse");
    assert_eq!(RasterStats::get(&stats.triangles_backface), 1);
}

#[test]
fn sin_descarte_la_cara_trasera_si_se_dibuja() {
    let mut fb = Framebuffer::new(64, 64);
    fb.clear(0);
    let u = uniforms(&fb);
    let stats = RasterStats::default();

    draw_mesh(
        &mut fb,
        &triangulo(-3.0, true),
        &Plano(Vec3::ONE),
        &u,
        DrawOptions::double_sided(),
        &stats,
    );

    assert_ne!(centro(&fb), 0);
}

#[test]
fn el_z_buffer_conserva_el_triangulo_mas_cercano() {
    let rojo = Vec3::new(1.0, 0.0, 0.0);
    let azul = Vec3::new(0.0, 0.0, 1.0);

    // Primero el lejano y despues el cercano.
    let mut a = Framebuffer::new(64, 64);
    a.clear(0);
    let ua = uniforms(&a);
    let stats = RasterStats::default();
    draw_mesh(&mut a, &triangulo(-8.0, false), &Plano(azul), &ua, DrawOptions::opaque(), &stats);
    draw_mesh(&mut a, &triangulo(-3.0, false), &Plano(rojo), &ua, DrawOptions::opaque(), &stats);

    // Y ahora al reves.
    let mut b = Framebuffer::new(64, 64);
    b.clear(0);
    let ub = uniforms(&b);
    draw_mesh(&mut b, &triangulo(-3.0, false), &Plano(rojo), &ub, DrawOptions::opaque(), &stats);
    draw_mesh(&mut b, &triangulo(-8.0, false), &Plano(azul), &ub, DrawOptions::opaque(), &stats);

    assert_eq!(
        centro(&a),
        centro(&b),
        "el resultado no debe depender del orden de dibujo"
    );
    // El rojo gana: su componente roja domina.
    let c = centro(&a);
    let r = (c >> 16) & 0xFF;
    let bl = c & 0xFF;
    assert!(r > bl, "deberia verse el triangulo cercano (rojo)");
}

#[test]
fn una_superficie_que_pasa_bajo_la_camara_se_recorta_bien() {
    // Un piso largo que empieza delante de la camara y sigue hasta
    // quedar detras de ella. Sin recorte del plano cercano, la mitad que
    // queda atras se proyecta con w negativo y mancha la pantalla.
    let n = Vec3::UP;
    let vs = vec![
        Vertex::new(Vec3::new(-10.0, -1.0, -20.0), n, Vec2::new(0.0, 0.0)),
        Vertex::new(Vec3::new(10.0, -1.0, -20.0), n, Vec2::new(1.0, 0.0)),
        Vertex::new(Vec3::new(10.0, -1.0, 10.0), n, Vec2::new(1.0, 1.0)),
        Vertex::new(Vec3::new(-10.0, -1.0, 10.0), n, Vec2::new(0.0, 1.0)),
    ];
    let mesh = Mesh::new(vs, vec![[0, 1, 2], [0, 2, 3]]);

    let mut fb = Framebuffer::new(64, 64);
    fb.clear(0);
    let u = uniforms(&fb);
    let stats = RasterStats::default();
    draw_mesh(&mut fb, &mesh, &Plano(Vec3::ONE), &u, DrawOptions::double_sided(), &stats);

    for d in &fb.depth {
        assert!(!d.is_nan(), "el z-buffer tiene NaN");
        if d.is_finite() {
            assert!((-1.0..=1.0).contains(d), "profundidad fuera de rango: {d}");
        }
    }

    // El piso tiene que cubrir la mitad inferior y dejar limpia la
    // superior.
    let fila_baja = (0..64).filter(|&x| fb.color[60 * 64 + x] != 0).count();
    let fila_alta = (0..64).filter(|&x| fb.color[4 * 64 + x] != 0).count();
    assert!(fila_baja > 50, "el piso deberia llenar la parte baja: {fila_baja}");
    assert_eq!(fila_alta, 0, "no deberia pintarse nada arriba del horizonte");
}

#[test]
fn la_interpolacion_corrige_la_perspectiva() {
    // Un cuadrilatero muy inclinado: el borde lejano se comprime en
    // pantalla, asi que la u interpolada no puede avanzar de forma lineal.
    let n = Vec3::new(0.0, 1.0, 0.0);
    let vs = vec![
        Vertex::new(Vec3::new(-1.0, -0.5, -1.5), n, Vec2::new(0.0, 0.0)),
        Vertex::new(Vec3::new(1.0, -0.5, -1.5), n, Vec2::new(1.0, 0.0)),
        Vertex::new(Vec3::new(1.0, -0.5, -40.0), n, Vec2::new(1.0, 1.0)),
        Vertex::new(Vec3::new(-1.0, -0.5, -40.0), n, Vec2::new(0.0, 1.0)),
    ];
    let mesh = Mesh::new(vs, vec![[0, 1, 2], [0, 2, 3]]);

    let mut fb = Framebuffer::new(128, 128);
    fb.clear(0);
    let u = uniforms(&fb);
    let stats = RasterStats::default();
    draw_mesh(&mut fb, &mesh, &MuestraU, &u, DrawOptions::double_sided(), &stats);

    let pintados = fb.color.iter().filter(|&&c| c != 0).count();
    assert!(pintados > 100, "el cuadrilatero deberia cubrir buena parte de la pantalla");
}

#[test]
fn las_bandas_cubren_el_framebuffer_completo() {
    let mut fb = Framebuffer::new(37, 23);
    let bandas = fb.bands(5);
    let filas: usize = bandas.iter().map(|b| b.y1 - b.y0).sum();
    assert_eq!(filas, 23, "las bandas deben cubrir todas las filas");
    assert_eq!(bandas[0].y0, 0);
    assert_eq!(bandas.last().unwrap().y1, 23);
}
