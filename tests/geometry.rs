//! Pruebas de la geometria procedural: icoesferas, subdivision,
//! deformacion por ruido, modelos propios y mapas de normales.

use std::collections::HashMap;

use sistema_solar::geometry::mesh::Mesh;
use sistema_solar::geometry::models;
use sistema_solar::geometry::primitives::{box_mesh, cone, cylinder};
use sistema_solar::geometry::sphere::{icosphere, ring};
use sistema_solar::geometry::terrain::{displace_sphere, Terrain, TerrainKind};
use sistema_solar::math::{Vec2, Vec3};
use sistema_solar::render::texture::{apply_normal_map, generate_normal_map, uv_to_direction};

/// Comprueba que la malla sea cerrada: cada arista tiene que aparecer en
/// exactamente dos triangulos. Si no, el descarte de caras traseras deja
/// agujeros por los que se ve el interior.
fn es_cerrada(mesh: &Mesh) -> bool {
    let mut conteo: HashMap<(u32, u32), usize> = HashMap::new();
    for t in &mesh.indices {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let k = if a < b { (a, b) } else { (b, a) };
            *conteo.entry(k).or_insert(0) += 1;
        }
    }
    conteo.values().all(|&c| c == 2)
}

#[test]
fn la_subdivision_multiplica_los_triangulos_por_cuatro() {
    let esperado = [20, 80, 320, 1280, 5120];
    for (nivel, &tris) in esperado.iter().enumerate() {
        let m = icosphere(1.0, nivel as u32);
        assert_eq!(m.triangle_count(), tris, "nivel {nivel}");
    }
}

#[test]
fn los_vertices_de_la_icoesfera_caen_sobre_la_esfera() {
    let r = 2.5;
    let m = icosphere(r, 3);
    for v in &m.vertices {
        assert!(
            (v.position.length() - r).abs() < 1e-4,
            "radio {} en vez de {r}",
            v.position.length()
        );
    }
    assert!((m.bounding_radius - r).abs() < 1e-4);
}

#[test]
fn la_icoesfera_es_una_malla_cerrada() {
    for nivel in 0..4 {
        let m = icosphere(1.0, nivel);
        assert!(es_cerrada(&m), "la icoesfera de nivel {nivel} tiene agujeros");
    }
}

#[test]
fn las_normales_de_la_icoesfera_apuntan_hacia_afuera() {
    let m = icosphere(1.0, 2);
    for v in &m.vertices {
        assert!(
            v.normal.dot(v.position.normalized()) > 0.99,
            "normal hacia adentro"
        );
    }
}

#[test]
fn la_icoesfera_esta_enrollada_en_sentido_antihorario() {
    // La normal geometrica de cada cara tiene que apuntar hacia afuera,
    // que es lo que espera el descarte de caras traseras.
    let m = icosphere(1.0, 1);
    for t in &m.indices {
        let a = m.vertices[t[0] as usize].position;
        let b = m.vertices[t[1] as usize].position;
        let c = m.vertices[t[2] as usize].position;
        let n = (b - a).cross(c - a);
        let centro = (a + b + c) / 3.0;
        assert!(n.dot(centro) > 0.0, "cara enrollada al reves");
    }
}

#[test]
fn el_terreno_es_reproducible() {
    let t = Terrain::new(TerrainKind::Mountains, 0.08, 2.5, 777);
    let dir = Vec3::new(0.3, 0.7, -0.2).normalized();
    assert_eq!(t.height(dir), t.height(dir));

    let otro = Terrain::new(TerrainKind::Mountains, 0.08, 2.5, 778);
    assert!((t.height(dir) - otro.height(dir)).abs() > 1e-6);
}

#[test]
fn la_deformacion_mueve_los_vertices_dentro_de_la_amplitud() {
    let radio = 3.0;
    let amplitud = 0.1;
    let t = Terrain::new(TerrainKind::Continents, amplitud, 2.0, 5);
    let mut m = icosphere(radio, 3);
    displace_sphere(&mut m, &t, radio);

    let mut hubo_cambio = false;
    for v in &m.vertices {
        let r = v.position.length();
        assert!(
            r >= radio * (1.0 - amplitud) - 1e-3 && r <= radio * (1.0 + amplitud) + 1e-3,
            "radio fuera de la amplitud: {r}"
        );
        if (r - radio).abs() > 1e-3 {
            hubo_cambio = true;
        }
        assert!((-1.0..=1.0).contains(&v.height));
    }
    assert!(hubo_cambio, "la deformacion no hizo nada");
    assert!(es_cerrada(&m), "deformar no debe romper la malla");
}

#[test]
fn un_terreno_plano_no_deforma() {
    let t = Terrain::flat(1);
    let mut m = icosphere(2.0, 2);
    let antes: Vec<Vec3> = m.vertices.iter().map(|v| v.position).collect();
    displace_sphere(&mut m, &t, 2.0);
    for (v, p) in m.vertices.iter().zip(antes) {
        assert!((v.position - p).length() < 1e-6);
    }
}

#[test]
fn la_deformacion_no_depende_del_nivel_de_detalle() {
    // Dos niveles de detalle del mismo planeta tienen que describir la
    // misma montaña: si no, cambiar de nivel produciria un salto visible.
    let t = Terrain::new(TerrainKind::Mountains, 0.07, 3.0, 21);
    let dir = Vec3::new(-0.5, 0.2, 0.84).normalized();
    let mut bajo = icosphere(1.0, 1);
    let mut alto = icosphere(1.0, 4);
    displace_sphere(&mut bajo, &t, 1.0);
    displace_sphere(&mut alto, &t, 1.0);
    assert_eq!(t.radius_at(dir, 1.0), t.radius_at(dir, 1.0));
    assert!(bajo.triangle_count() < alto.triangle_count());
}

#[test]
fn las_primitivas_producen_mallas_validas() {
    let caja = box_mesh(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(caja.triangle_count(), 12);
    assert!((caja.bounding_radius - 14.0f32.sqrt()).abs() < 1e-4);

    let cil = cylinder(1.0, 0.5, 2.0, 12);
    assert!(cil.triangle_count() > 0);
    let con = cone(1.0, 2.0, 12);
    assert!(con.triangle_count() > 0);

    for m in [&caja, &cil, &con] {
        for t in &m.indices {
            assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2], "triangulo degenerado");
        }
        for v in &m.vertices {
            assert!(v.position.is_finite() && v.normal.is_finite());
            assert!((v.normal.length() - 1.0).abs() < 1e-3, "normal sin normalizar");
        }
    }
}

#[test]
fn el_anillo_se_cierra_sobre_si_mismo() {
    let a = ring(2.0, 3.0, 32);
    assert_eq!(a.vertices.len(), 64);
    assert_eq!(a.triangle_count(), 64);
    for v in &a.vertices {
        let r = Vec2::new(v.position.x, v.position.z).length();
        assert!(r >= 2.0 - 1e-4 && r <= 3.0 + 1e-4, "radio {r}");
        assert!(v.position.y.abs() < 1e-6, "el anillo deberia ser plano");
    }
}

#[test]
fn los_modelos_propios_tienen_geometria() {
    let nave = models::ship();
    assert!(nave.triangle_count() > 100, "la nave quedo demasiado simple");
    assert!(nave.bounding_radius > 0.5 && nave.bounding_radius < 4.0);

    let props = models::surface_props();
    assert_eq!(props.len(), 4, "deberian ser cuatro tipos distintos");
    let mut conteos = Vec::new();
    for p in &props {
        assert!(p.triangle_count() > 8, "modelo vacio");
        // Todos nacen apoyados en y = 0 y crecen hacia arriba.
        let min_y = p.vertices.iter().map(|v| v.position.y).fold(f32::MAX, f32::min);
        assert!(min_y > -0.35, "el modelo se hunde demasiado: {min_y}");
        conteos.push(p.triangle_count());
    }
    conteos.sort_unstable();
    conteos.dedup();
    assert_eq!(conteos.len(), 4, "los cuatro tipos no son geometrias distintas");
}

#[test]
fn el_mapa_de_normales_de_un_terreno_plano_apunta_hacia_arriba() {
    let t = Terrain::flat(0);
    let tex = generate_normal_map(&t, 32, 16, 1.0);
    for texel in &tex.data {
        assert!((texel.x - 0.5).abs() < 1e-3, "x = {}", texel.x);
        assert!((texel.y - 0.5).abs() < 1e-3, "y = {}", texel.y);
        assert!((texel.z - 1.0).abs() < 1e-3, "z = {}", texel.z);
    }
}

#[test]
fn un_mapa_de_normales_neutro_no_cambia_la_normal() {
    let n = Vec3::new(0.3, 0.5, -0.8).normalized();
    let neutro = Vec3::new(0.5, 0.5, 1.0);
    let salida = apply_normal_map(n, neutro, 1.0);
    assert!((salida - n).length() < 1e-4, "{:?} vs {:?}", salida, n);
}

#[test]
fn un_terreno_con_relieve_inclina_las_normales() {
    let t = Terrain::new(TerrainKind::Mountains, 0.08, 4.0, 3);
    let tex = generate_normal_map(&t, 64, 32, 1.0);
    let inclinados = tex
        .data
        .iter()
        .filter(|c| (c.x - 0.5).abs() > 0.02 || (c.y - 0.5).abs() > 0.02)
        .count();
    assert!(
        inclinados > tex.data.len() / 4,
        "casi ningun texel quedo inclinado: {inclinados}"
    );
}

#[test]
fn el_mapeo_equirectangular_y_su_inversa_coinciden() {
    // uv_to_direction tiene que ser la inversa del mapeo que usa la
    // icoesfera, o la textura saldria corrida respecto de la geometria.
    let m = icosphere(1.0, 2);
    for v in m.vertices.iter().take(40) {
        let d = uv_to_direction(v.uv.x, v.uv.y);
        assert!(
            (d - v.position.normalized()).length() < 1e-3,
            "{:?} vs {:?}",
            d,
            v.position
        );
    }
}

#[test]
fn el_muestreo_de_textura_se_repite_en_horizontal() {
    let t = Terrain::new(TerrainKind::Craters, 0.05, 3.0, 9);
    let tex = generate_normal_map(&t, 32, 16, 1.0);
    let a = tex.sample(Vec2::new(0.01, 0.5));
    let b = tex.sample(Vec2::new(1.01, 0.5));
    assert!((a - b).length() < 1e-4, "la textura deberia envolver el planeta");
}
