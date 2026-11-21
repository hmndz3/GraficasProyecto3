//! Pruebas de los fragment shaders.
//!
//! No se compara contra imagenes de referencia: lo que se verifica es que
//! cada shader de un color valido, que el lado iluminado sea mas claro que
//! el oscuro, que los shaders animados cambien con el tiempo y que los
//! seis produzcan resultados distintos entre si.

use std::sync::Arc;

use sistema_solar::geometry::terrain::{Terrain, TerrainKind};
use sistema_solar::math::{Mat4, Vec2, Vec3};
use sistema_solar::noise::Noise;
use sistema_solar::render::pipeline::{Fragment, FragmentShader, Uniforms};
use sistema_solar::render::texture::generate_normal_map;
use sistema_solar::shaders::{
    GasShader, IceShader, OceanShader, RingShader, RockyShader, StarShader,
};

fn uniformes(t: f32) -> Uniforms {
    Uniforms {
        model: Mat4::identity(),
        view: Mat4::identity(),
        projection: Mat4::identity(),
        normal_matrix: Mat4::identity(),
        light_world: Vec3::new(0.0, 0.0, 0.0),
        camera_world: Vec3::new(0.0, 0.0, 30.0),
        time: t,
        object_scale: 1.0,
        tint: Vec3::ONE,
        param: 0.0,
    }
}

/// Fragmento sobre la superficie de un planeta puesto a 20 unidades del
/// sol, mirando hacia `dir`.
fn fragmento(dir: Vec3, altura: f32) -> Fragment {
    let dir = dir.normalized();
    let centro = Vec3::new(20.0, 0.0, 0.0);
    Fragment {
        world: centro + dir * 2.0,
        local: dir * 2.0,
        normal: dir,
        uv: Vec2::new(0.5 + dir.z.atan2(dir.x) / std::f32::consts::TAU, 0.5),
        height: altura,
        depth: 0.5,
    }
}

fn mapa() -> Arc<sistema_solar::render::texture::Texture> {
    let t = Terrain::new(TerrainKind::Mountains, 0.06, 3.0, 5);
    Arc::new(generate_normal_map(&t, 32, 16, 1.0))
}

fn rocoso() -> RockyShader {
    RockyShader {
        noise: Noise::new(5),
        normal_map: mapa(),
        normal_strength: 1.0,
        paleta: vec![
            (0.0, Vec3::new(0.18, 0.14, 0.12)),
            (0.5, Vec3::new(0.42, 0.31, 0.24)),
            (1.0, Vec3::new(0.72, 0.66, 0.58)),
        ],
        detalle: 12.0,
        especular: 0.05,
    }
}

fn oceanico() -> OceanShader {
    OceanShader {
        noise: Noise::new(9),
        normal_map: mapa(),
        normal_strength: 0.9,
        nivel_mar: 0.0,
        agua_profunda: Vec3::new(0.01, 0.05, 0.16),
        agua_baja: Vec3::new(0.05, 0.3, 0.42),
        paleta_tierra: vec![
            (0.0, Vec3::new(0.35, 0.42, 0.2)),
            (1.0, Vec3::new(0.8, 0.8, 0.82)),
        ],
        nubes: 0.6,
        velocidad_nubes: 0.02,
        color_atmosfera: Vec3::new(0.25, 0.45, 0.9),
    }
}

fn gaseoso() -> GasShader {
    GasShader {
        noise: Noise::new(3),
        bandas: 9.0,
        deriva: 0.05,
        turbulencia: 0.4,
        color_claro: Vec3::new(0.86, 0.74, 0.55),
        color_oscuro: Vec3::new(0.45, 0.3, 0.2),
        color_tormenta: Vec3::new(0.8, 0.3, 0.2),
        tormenta_lat: -0.25,
        tormenta_lon: 1.1,
        tormenta_tam: 0.5,
        color_atmosfera: Vec3::new(0.7, 0.6, 0.4),
    }
}

fn helado() -> IceShader {
    IceShader {
        noise: Noise::new(11),
        normal_map: mapa(),
        normal_strength: 0.7,
        escala_placas: 6.0,
        hielo_claro: Vec3::new(0.82, 0.9, 0.98),
        hielo_sombra: Vec3::new(0.45, 0.58, 0.72),
        color_grieta: Vec3::new(0.08, 0.16, 0.3),
        color_atmosfera: Vec3::new(0.5, 0.7, 1.0),
    }
}

fn color_valido(c: Vec3) -> bool {
    c.is_finite() && c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0
}

#[test]
fn todos_los_shaders_dan_colores_validos() {
    let u = uniformes(3.5);
    let star = StarShader::new(1);
    let rocky = rocoso();
    let ocean = oceanico();
    let gas = gaseoso();
    let ice = helado();

    let shaders: Vec<(&str, &dyn FragmentShader)> = vec![
        ("estrella", &star),
        ("rocoso", &rocky),
        ("oceanico", &ocean),
        ("gaseoso", &gas),
        ("helado", &ice),
    ];

    for (nombre, s) in shaders {
        for i in 0..64 {
            let a = i as f32 * 0.37;
            let dir = Vec3::new(a.sin(), (a * 1.7).cos(), (a * 0.9).sin());
            for h in [-0.8, -0.1, 0.0, 0.3, 0.9] {
                let c = s.shade(&fragmento(dir, h), &u);
                assert!(color_valido(c.xyz()), "{nombre} devolvio {:?}", c);
                assert!((0.0..=1.0).contains(&c.w), "{nombre} alpha = {}", c.w);
            }
        }
    }
}

#[test]
fn el_lado_iluminado_es_mas_claro_que_el_oscuro() {
    let u = uniformes(0.0);
    // El sol esta en el origen y el planeta en x = 20: la cara iluminada
    // mira hacia -x.
    let hacia_el_sol = Vec3::new(-1.0, 0.0, 0.0);
    let hacia_afuera = Vec3::new(1.0, 0.0, 0.0);

    let rocky = rocoso();
    let gas = gaseoso();
    let ice = helado();
    for (nombre, s) in [
        ("rocoso", &rocky as &dyn FragmentShader),
        ("gaseoso", &gas),
        ("helado", &ice),
    ] {
        let claro = rocky_luma(s.shade(&fragmento(hacia_el_sol, 0.2), &u).xyz());
        let oscuro = rocky_luma(s.shade(&fragmento(hacia_afuera, 0.2), &u).xyz());
        assert!(
            claro > oscuro * 3.0,
            "{nombre}: cara iluminada {claro} vs oscura {oscuro}"
        );
    }
}

fn rocky_luma(c: Vec3) -> f32 {
    c.x * 0.2126 + c.y * 0.7152 + c.z * 0.0722
}

#[test]
fn el_shader_de_la_estrella_cambia_con_el_tiempo() {
    let star = StarShader::new(42);
    let dir = Vec3::new(0.3, 0.6, -0.74);

    let a = star.shade(&fragmento(dir, 0.0), &uniformes(0.0)).xyz();
    let b = star.shade(&fragmento(dir, 0.0), &uniformes(12.0)).xyz();
    assert!(
        (a - b).length() > 0.02,
        "el plasma deberia haber fluido: {:?} vs {:?}",
        a,
        b
    );

    // Y con el mismo tiempo tiene que dar exactamente lo mismo.
    let c = star.shade(&fragmento(dir, 0.0), &uniformes(12.0)).xyz();
    assert_eq!(b, c);
}

#[test]
fn las_bandas_del_gigante_gaseoso_se_desplazan() {
    let gas = gaseoso();
    // Del lado iluminado: en la cara nocturna todo queda casi negro y
    // cualquier diferencia se pierde.
    let dir = Vec3::new(-0.6, 0.3, 0.74);
    let a = gas.shade(&fragmento(dir, 0.0), &uniformes(0.0)).xyz();
    let b = gas.shade(&fragmento(dir, 0.0), &uniformes(40.0)).xyz();
    assert!((a - b).length() > 0.01, "las bandas deberian haberse movido");
}

#[test]
fn las_nubes_del_planeta_oceanico_se_mueven() {
    let ocean = oceanico();
    let dir = Vec3::new(-0.5, 0.4, 0.76);
    let a = ocean.shade(&fragmento(dir, 0.3), &uniformes(0.0)).xyz();
    let b = ocean.shade(&fragmento(dir, 0.3), &uniformes(60.0)).xyz();
    assert!((a - b).length() > 0.005, "las nubes deberian haberse desplazado");
}

#[test]
fn el_oceano_y_la_tierra_se_ven_distintos() {
    let ocean = oceanico();
    let u = uniformes(0.0);
    let dir = Vec3::new(-1.0, 0.0, 0.0);
    let agua = ocean.shade(&fragmento(dir, -0.5), &u).xyz();
    let tierra = ocean.shade(&fragmento(dir, 0.5), &u).xyz();
    assert!(
        (agua - tierra).length() > 0.05,
        "el nivel del mar deberia separar dos materiales"
    );
    // El agua profunda tiene que tirar a azul.
    assert!(agua.z > agua.x, "el agua deberia ser azulada: {:?}", agua);
}

#[test]
fn los_shaders_de_planeta_no_se_parecen_entre_si() {
    let u = uniformes(1.0);
    let dir = Vec3::new(-0.7, 0.3, 0.65);
    let rocky = rocoso();
    let ocean = oceanico();
    let gas = gaseoso();
    let ice = helado();
    let star = StarShader::new(2);

    let colores = vec![
        rocky.shade(&fragmento(dir, 0.3), &u).xyz(),
        ocean.shade(&fragmento(dir, 0.3), &u).xyz(),
        gas.shade(&fragmento(dir, 0.3), &u).xyz(),
        ice.shade(&fragmento(dir, 0.3), &u).xyz(),
        star.shade(&fragmento(dir, 0.3), &u).xyz(),
    ];

    for i in 0..colores.len() {
        for j in (i + 1)..colores.len() {
            assert!(
                (colores[i] - colores[j]).length() > 0.03,
                "los shaders {i} y {j} dan practicamente el mismo color"
            );
        }
    }
}

#[test]
fn los_anillos_dejan_huecos() {
    let anillo = RingShader {
        noise: Noise::new(4),
        color_interno: Vec3::new(0.7, 0.62, 0.5),
        color_externo: Vec3::new(0.45, 0.42, 0.4),
        frecuencia: 18.0,
        hueco: 0.55,
        densidad: 1.0,
    };
    let u = uniformes(0.0);

    let mut minimo: f32 = 1.0;
    let mut maximo: f32 = 0.0;
    for i in 0..200 {
        let t = i as f32 / 199.0;
        let mut f = fragmento(Vec3::new(1.0, 0.0, 0.0), 0.0);
        f.uv = Vec2::new(t, 0.0);
        f.normal = Vec3::UP;
        let c = anillo.shade(&f, &u);
        minimo = minimo.min(c.w);
        maximo = maximo.max(c.w);
    }
    assert!(minimo < 0.1, "los anillos deberian tener huecos: {minimo}");
    assert!(maximo > 0.4, "los anillos deberian tener zonas densas: {maximo}");
}
