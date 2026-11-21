//! Pruebas del algebra lineal del renderizador.

use sistema_solar::math::{to_radians, Mat4, Vec3, Vec4};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn close_vec(a: Vec3, b: Vec3) -> bool {
    close(a.x, b.x) && close(a.y, b.y) && close(a.z, b.z)
}

#[test]
fn identidad_no_cambia_un_punto() {
    let p = Vec3::new(3.0, -2.0, 7.5);
    assert!(close_vec(Mat4::identity().mul_point(p), p));
}

#[test]
fn producto_de_matrices_es_asociativo_con_el_punto() {
    let a = Mat4::rotation_y(0.7);
    let b = Mat4::translation(Vec3::new(1.0, 2.0, 3.0));
    let p = Vec3::new(0.5, -1.0, 2.0);
    assert!(close_vec((a * b).mul_point(p), a.mul_point(b.mul_point(p))));
}

#[test]
fn traslacion_no_afecta_direcciones() {
    let m = Mat4::translation(Vec3::new(10.0, -4.0, 2.0));
    let d = Vec3::new(0.0, 0.0, 1.0);
    assert!(close_vec(m.mul_dir(d), d));
}

#[test]
fn rotacion_en_eje_arbitrario_equivale_a_la_rotacion_en_y() {
    let a = Mat4::rotation_axis(Vec3::UP, 1.1);
    let b = Mat4::rotation_y(1.1);
    let p = Vec3::new(1.0, 0.5, -2.0);
    assert!(close_vec(a.mul_point(p), b.mul_point(p)));
}

#[test]
fn rotacion_preserva_la_longitud() {
    let m = Mat4::rotation_axis(Vec3::new(1.0, 2.0, -0.5), 2.3);
    let p = Vec3::new(3.0, -1.0, 4.0);
    assert!(close(m.mul_point(p).length(), p.length()));
}

#[test]
fn look_at_pone_el_objetivo_sobre_el_eje_z_negativo() {
    let eye = Vec3::new(5.0, 2.0, -3.0);
    let target = Vec3::new(0.0, 0.0, 0.0);
    let view = Mat4::look_at(eye, target, Vec3::UP);

    // El ojo cae en el origen del espacio de camara.
    assert!(close_vec(view.mul_point(eye), Vec3::ZERO));

    // El objetivo queda justo enfrente, es decir en z negativa.
    let t = view.mul_point(target);
    assert!(close(t.x, 0.0), "x = {}", t.x);
    assert!(close(t.y, 0.0), "y = {}", t.y);
    assert!(t.z < 0.0, "el objetivo deberia quedar en z negativa: {}", t.z);
}

#[test]
fn perspectiva_manda_el_plano_cercano_a_menos_uno() {
    let proj = Mat4::perspective(to_radians(60.0), 16.0 / 9.0, 0.1, 100.0);
    let near = proj.mul_vec4(Vec4::new(0.0, 0.0, -0.1, 1.0));
    assert!(close(near.z / near.w, -1.0), "z = {}", near.z / near.w);

    let far = proj.mul_vec4(Vec4::new(0.0, 0.0, -100.0, 1.0));
    assert!(close(far.z / far.w, 1.0), "z = {}", far.z / far.w);
}

#[test]
fn perspectiva_deja_w_positivo_delante_de_la_camara() {
    let proj = Mat4::perspective(to_radians(60.0), 1.0, 0.1, 100.0);
    let v = proj.mul_vec4(Vec4::new(1.0, 1.0, -5.0, 1.0));
    assert!(v.w > 0.0, "w = {}", v.w);
}

#[test]
fn inversa_rigida_deshace_la_transformacion() {
    let m = Mat4::translation(Vec3::new(4.0, -1.0, 2.0))
        * Mat4::rotation_axis(Vec3::new(0.3, 1.0, 0.2), 0.9)
        * Mat4::uniform_scale(2.5);
    let inv = m.inverse_rigid();
    let p = Vec3::new(1.0, 2.0, 3.0);
    assert!(close_vec(inv.mul_point(m.mul_point(p)), p));
}

#[test]
fn matriz_de_normales_corrige_la_escala_no_uniforme() {
    // Con escala no uniforme, usar la matriz de modelo tal cual deforma la
    // normal. La matriz de normales debe mantenerla perpendicular.
    let m = Mat4::scale(Vec3::new(1.0, 4.0, 1.0));
    let tangente = Vec3::new(1.0, 1.0, 0.0).normalized();
    let normal = Vec3::new(-1.0, 1.0, 0.0).normalized();

    let t2 = m.mul_dir(tangente);
    let n_mal = m.mul_dir(normal).normalized();
    let n_bien = m.normal_matrix().mul_dir(normal).normalized();

    assert!(n_mal.dot(t2.normalized()).abs() > 0.1, "el caso de prueba no es util");
    assert!(
        n_bien.dot(t2.normalized()).abs() < 1e-3,
        "la normal corregida deberia seguir siendo perpendicular: {}",
        n_bien.dot(t2.normalized())
    );
}

#[test]
fn normalizar_un_vector_nulo_no_produce_nan() {
    assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
}

#[test]
fn producto_cruz_sigue_la_regla_de_la_mano_derecha() {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    assert!(close_vec(x.cross(y), Vec3::new(0.0, 0.0, 1.0)));
}
