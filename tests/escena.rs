//! Pruebas de la escena: orbitas sobre el plano ecliptico, giro propio,
//! niveles de detalle, descarte por frustum, siembra de objetos,
//! gravedad y colisiones.

use sistema_solar::geometry::terrain::{Terrain, TerrainKind};
use sistema_solar::math::{to_radians, Mat4, Vec3};
use sistema_solar::render::framebuffer::Framebuffer;
use sistema_solar::render::frustum::Frustum;
use sistema_solar::render::pipeline::RasterStats;
use sistema_solar::scene::camera::{Camera, CameraMode};
use sistema_solar::scene::props::{scatter, PropRecipe};
use sistema_solar::scene::ship::{FlightMode, Ship, ShipInput};
use sistema_solar::scene::system::{RenderToggles, SolarSystem};
use sistema_solar::scene::tour::Tour;

const SEMILLA: u32 = 20_260_413;

fn sistema() -> SolarSystem {
    let mut s = SolarSystem::new(SEMILLA);
    s.update(0.0);
    s
}

#[test]
fn los_planetas_orbitan_sobre_el_plano_ecliptico() {
    let mut s = sistema();
    s.update(37.0);
    for b in &s.bodies {
        // Solo los planetas: las lunas si tienen inclinacion propia.
        if b.parent == Some(0) {
            assert!(
                b.position.y.abs() < 1e-4,
                "{} se salio del plano: y = {}",
                b.name,
                b.position.y
            );
            assert_eq!(b.orbit.inclination, 0.0, "{} tiene inclinacion", b.name);
        }
    }
}

#[test]
fn la_simulacion_depende_del_tiempo_y_no_de_los_cuadros() {
    // El mismo tiempo simulado en un paso grande o en muchos pequeños
    // tiene que dejar los cuerpos en el mismo lugar.
    let mut a = SolarSystem::new(SEMILLA);
    a.update(3.0);

    let mut b = SolarSystem::new(SEMILLA);
    for _ in 0..300 {
        b.update(0.01);
    }

    for (x, y) in a.bodies.iter().zip(b.bodies.iter()) {
        assert!(
            (x.position - y.position).length() < 1e-2,
            "{} quedo en {:?} contra {:?}",
            x.name,
            x.position,
            y.position
        );
    }
}

#[test]
fn las_lunas_siguen_a_su_planeta() {
    let mut s = sistema();
    s.update(60.0);
    for (i, b) in s.bodies.iter().enumerate() {
        let Some(p) = b.parent else { continue };
        if p == 0 {
            continue;
        }
        let d = (b.position - s.bodies[p].position).length();
        assert!(
            (d - b.orbit.radius).abs() < 0.3,
            "la luna {i} quedo a {d} de su planeta en vez de {}",
            b.orbit.radius
        );
    }
}

#[test]
fn los_cuerpos_giran_sobre_su_eje() {
    let mut s = sistema();
    let antes: Vec<f32> = s.bodies.iter().map(|b| b.spin).collect();
    s.update(2.0);
    for (b, a) in s.bodies.iter().zip(antes) {
        if b.spin_speed > 0.0 {
            assert!((b.spin - a).abs() > 1e-3, "{} no giro", b.name);
        }
    }
}

#[test]
fn hay_al_menos_cinco_planetas_y_una_estrella() {
    let s = sistema();
    let planetas = s.bodies.iter().filter(|b| b.parent == Some(0)).count();
    let lunas = s.bodies.iter().filter(|b| matches!(b.parent, Some(p) if p != 0)).count();
    assert!(planetas >= 5, "solo hay {planetas} planetas");
    assert!(lunas >= 2, "solo hay {lunas} lunas");
    assert!(s.bodies[0].emissive, "la estrella deberia emitir luz");
}

#[test]
fn cada_cuerpo_tiene_varios_niveles_de_detalle() {
    let s = sistema();
    for b in &s.bodies {
        assert!(b.lods.len() >= 3, "{} tiene pocos niveles", b.name);
        // Cada nivel tiene que ser mas simple que el anterior.
        for par in b.lods.windows(2) {
            assert!(
                par[0].triangle_count() > par[1].triangle_count(),
                "{} tiene niveles fuera de orden",
                b.name
            );
        }
    }
}

#[test]
fn el_nivel_de_detalle_baja_con_la_distancia() {
    let s = sistema();
    let b = &s.bodies[2];
    let cerca = b.position + Vec3::new(b.radius * 2.0, 0.0, 0.0);
    let lejos = b.position + Vec3::new(400.0, 0.0, 0.0);

    assert_eq!(b.lod_index(cerca, true), 0);
    assert_eq!(b.lod_index(lejos, true), b.lods.len() - 1);
    // Con los niveles apagados siempre se usa el mas detallado.
    assert_eq!(b.lod_index(lejos, false), 0);
}

#[test]
fn el_frustum_descarta_lo_que_queda_atras() {
    let vista = Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP);
    let proy = Mat4::perspective(to_radians(60.0), 16.0 / 9.0, 0.1, 500.0);
    let f = Frustum::from_view_projection(&(proy * vista));

    assert!(f.sphere_visible(Vec3::new(0.0, 0.0, -10.0), 1.0), "lo de enfrente deberia verse");
    assert!(!f.sphere_visible(Vec3::new(0.0, 0.0, 10.0), 1.0), "lo de atras deberia descartarse");
    assert!(!f.sphere_visible(Vec3::new(200.0, 0.0, -10.0), 1.0), "lo de un lado deberia descartarse");
    // Una esfera grande apenas fuera del borde sigue tocando el volumen.
    assert!(f.sphere_visible(Vec3::new(0.0, 0.0, 3.0), 6.0));
    assert!(!f.sphere_visible(Vec3::new(0.0, 0.0, -600.0), 1.0), "mas alla del plano lejano");
}

#[test]
fn la_siembra_de_objetos_es_reproducible() {
    let terreno = Terrain::new(TerrainKind::Continents, 0.04, 2.0, 7);
    let receta = PropRecipe { seed: 1234, max_instances: 80, ..Default::default() };

    let a = scatter(&receta, &terreno, 2.0);
    let b = scatter(&receta, &terreno, 2.0);
    assert_eq!(a.len(), b.len());
    assert!(!a.is_empty(), "no se sembro nada");
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.kind, y.kind);
        assert_eq!(x.direction, y.direction);
        assert_eq!(x.yaw, y.yaw);
    }

    // Con otra semilla, otras posiciones.
    let otra = PropRecipe { seed: 1235, ..receta };
    let c = scatter(&otra, &terreno, 2.0);
    assert!(a[0].direction != c[0].direction);
}

#[test]
fn los_objetos_quedan_sobre_la_superficie_y_separados() {
    let terreno = Terrain::new(TerrainKind::Mountains, 0.07, 2.5, 11);
    let receta = PropRecipe {
        seed: 99,
        max_instances: 120,
        min_separation: 0.09,
        ..Default::default()
    };
    let props = scatter(&receta, &terreno, 3.0);
    assert!(props.len() > 20, "se sembraron muy pocos: {}", props.len());

    let cos_min = receta.min_separation.cos();
    for (i, p) in props.iter().enumerate() {
        assert!((p.direction.length() - 1.0).abs() < 1e-4);
        let esperado = terreno.radius_at(p.direction, 3.0);
        assert!(
            (p.radius - esperado).abs() < 0.2,
            "el objeto {i} quedo a {} y la superficie esta en {esperado}",
            p.radius
        );
        for q in props.iter().skip(i + 1) {
            assert!(
                p.direction.dot(q.direction) <= cos_min + 1e-5,
                "dos objetos quedaron encimados"
            );
        }
    }
}

#[test]
fn los_objetos_se_paran_sobre_la_curva_del_planeta() {
    let terreno = Terrain::new(TerrainKind::Continents, 0.05, 2.0, 3);
    let receta = PropRecipe { seed: 55, max_instances: 40, ..Default::default() };
    let props = scatter(&receta, &terreno, 2.0);

    for p in &props {
        let m = p.local_matrix();
        // La columna del eje Y de la matriz es la vertical del objeto y
        // tiene que coincidir con la normal de la superficie.
        let arriba = m.mul_dir(Vec3::UP).normalized();
        assert!(
            arriba.dot(p.direction) > 0.999,
            "el objeto quedo torcido: {:?} contra {:?}",
            arriba,
            p.direction
        );
        // Y la base tiene que apoyarse a la altura del terreno.
        let base = m.mul_point(Vec3::ZERO);
        assert!((base.length() - p.radius).abs() < 1e-3);
    }
}

#[test]
fn la_nave_no_atraviesa_un_planeta() {
    let s = sistema();
    let planeta = &s.bodies[2];
    let mut nave = Ship::default();

    // Se lanza la nave directo al centro del planeta.
    nave.position = planeta.position + Vec3::new(planeta.outer_radius() + 3.0, 0.0, 0.0);
    nave.forward = Vec3::new(-1.0, 0.0, 0.0);
    nave.right = Vec3::new(0.0, 0.0, 1.0);
    nave.up = Vec3::UP;
    nave.velocity = Vec3::new(-80.0, 0.0, 0.0);

    let entrada = ShipInput { thrust: 1.0, ..Default::default() };
    for _ in 0..400 {
        nave.update(&entrada, 1.0 / 60.0, None);
        s.aplicar_fisica(&mut nave, 1.0 / 60.0, false);
        let d = (nave.position - planeta.position).length();
        assert!(
            d >= planeta.radius * (1.0 - planeta.terrain.amplitude) - 0.01,
            "la nave entro al planeta: {d}"
        );
    }
}

#[test]
fn la_colision_devuelve_la_nave_afuera() {
    let mut nave = Ship::default();
    nave.position = Vec3::new(0.5, 0.0, 0.0);
    nave.velocity = Vec3::new(-10.0, 0.0, 0.0);
    let choco = nave.resolve_collision(Vec3::ZERO, 2.0, Vec3::ZERO);
    assert!(choco);
    assert!((nave.position.length() - (2.0 + nave.radius)).abs() < 1e-3);
    // Ya no se mueve hacia adentro.
    assert!(nave.velocity.dot(nave.position.normalized()) >= -1e-3);
}

#[test]
fn la_gravedad_atrae_la_nave() {
    let mut nave = Ship::default();
    nave.position = Vec3::new(30.0, 0.0, 0.0);
    nave.velocity = Vec3::ZERO;
    nave.apply_gravity(Vec3::ZERO, 1000.0, 0.5, 2.2);
    assert!(nave.velocity.x < 0.0, "deberia caer hacia el centro");

    // Mas lejos, menos fuerza: ley del inverso del cuadrado.
    let mut lejos = Ship::default();
    lejos.position = Vec3::new(60.0, 0.0, 0.0);
    lejos.velocity = Vec3::ZERO;
    lejos.apply_gravity(Vec3::ZERO, 1000.0, 0.5, 2.2);
    assert!(lejos.velocity.length() < nave.velocity.length() * 0.4);
}

#[test]
fn los_controles_de_la_nave_son_progresivos() {
    let mut nave = Ship::default();
    let entrada = ShipInput { thrust: 1.0, ..Default::default() };

    nave.update(&entrada, 1.0 / 60.0, None);
    let primero = nave.throttle;
    assert!(primero > 0.0 && primero < 0.2, "el acelerador salto de golpe: {primero}");

    for _ in 0..180 {
        nave.update(&entrada, 1.0 / 60.0, None);
    }
    assert!(nave.throttle > 0.95, "el acelerador no llego al tope: {}", nave.throttle);
}

#[test]
fn el_vuelo_no_depende_de_los_cuadros_por_segundo() {
    // Dos maquinas de distinta velocidad tienen que terminar en
    // practicamente el mismo punto despues del mismo tiempo.
    let entrada = ShipInput { thrust: 1.0, yaw: 0.4, ..Default::default() };

    let mut rapida = Ship::default();
    for _ in 0..240 {
        rapida.update(&entrada, 1.0 / 120.0, None);
    }

    let mut lenta = Ship::default();
    for _ in 0..60 {
        lenta.update(&entrada, 1.0 / 30.0, None);
    }

    // Lo que queda de diferencia es error de integracion, que se reduce
    // al achicar el paso; no es una dependencia del numero de cuadros.
    let d = (rapida.position - lenta.position).length();
    let recorrido = rapida.position.length().max(1.0);
    assert!(
        d < recorrido * 0.03,
        "las trayectorias se separaron {d} sobre un recorrido de {recorrido}"
    );
}

#[test]
fn el_modo_local_nivela_la_nave_con_la_superficie() {
    let mut nave = Ship::default();
    nave.mode = FlightMode::Local;
    nave.position = Vec3::new(0.0, 0.0, 10.0);
    nave.forward = Vec3::new(1.0, 0.0, 0.0);
    nave.right = Vec3::new(0.0, -1.0, 0.0);
    nave.up = Vec3::new(0.0, 0.0, 1.0);

    // La vertical local apunta hacia +z; la nave deberia alinearse.
    let vertical = Vec3::new(0.0, 0.0, 1.0);
    for _ in 0..240 {
        nave.update(&ShipInput::default(), 1.0 / 60.0, Some(vertical));
    }
    assert!(nave.up.dot(vertical) > 0.95, "no se nivelo: {:?}", nave.up);
}

#[test]
fn la_camara_cambia_de_modo_en_ciclo() {
    let mut m = CameraMode::Persecucion;
    let mut vistos = vec![m];
    for _ in 0..3 {
        m = m.siguiente();
        vistos.push(m);
    }
    assert_eq!(m.siguiente(), CameraMode::Persecucion);
    vistos.sort_by_key(|x| x.nombre());
    vistos.dedup_by_key(|x| x.nombre());
    assert_eq!(vistos.len(), 4, "deberian ser cuatro modos distintos");
}

#[test]
fn la_camara_de_persecucion_queda_detras_de_la_nave() {
    let mut nave = Ship::default();
    nave.position = Vec3::new(10.0, 0.0, 0.0);
    nave.forward = Vec3::new(1.0, 0.0, 0.0);
    nave.right = Vec3::new(0.0, 0.0, -1.0);
    nave.up = Vec3::UP;

    let mut c = Camera::default();
    for _ in 0..240 {
        c.follow(&nave, 1.0 / 60.0);
    }
    let detras = (nave.position - c.position).normalized();
    assert!(detras.dot(nave.forward) > 0.8, "la camara no quedo detras");
    assert!(c.position.y > nave.position.y, "deberia quedar un poco arriba");
}

#[test]
fn dibujar_la_escena_llena_el_framebuffer() {
    let s = sistema();
    let mut fb = Framebuffer::new(320, 180);
    let mut c = Camera::default();
    c.position = Vec3::new(0.0, 18.0, 60.0);
    c.target = Vec3::ZERO;
    let nave = Ship::default();
    let stats = RasterStats::default();

    s.draw(&mut fb, &c, &nave, true, &RenderToggles::default(), &stats);

    let pintados = fb.color.iter().filter(|&&p| p != 0).count();
    assert!(pintados > fb.color.len() / 2, "el fondo deberia cubrir la pantalla");
    assert!(
        RasterStats::get(&stats.triangles_rasterized) > 100,
        "casi no se rasterizo nada"
    );
    for &c in &fb.color {
        assert!(c <= 0x00FF_FFFF, "color fuera de rango");
    }
}

#[test]
fn el_descarte_reduce_el_trabajo() {
    let s = sistema();
    let mut fb = Framebuffer::new(240, 135);
    let nave = Ship::default();

    // Camara mirando al vacio, lejos de todo.
    let mut c = Camera::default();
    c.position = Vec3::new(0.0, 300.0, 0.0);
    c.target = Vec3::new(0.0, 900.0, 0.0);

    let con = RasterStats::default();
    s.draw(&mut fb, &c, &nave, false, &RenderToggles::default(), &con);

    let sin = RasterStats::default();
    let toggles = RenderToggles { frustum_culling: false, ..Default::default() };
    s.draw(&mut fb, &c, &nave, false, &toggles, &sin);

    assert!(
        RasterStats::get(&con.triangles_submitted) < RasterStats::get(&sin.triangles_submitted),
        "el descarte deberia enviar menos triangulos al rasterizador"
    );
    assert!(RasterStats::get(&con.meshes_culled) > 0);
}

#[test]
fn los_niveles_de_detalle_reducen_los_triangulos() {
    let s = sistema();
    let mut fb = Framebuffer::new(240, 135);
    let nave = Ship::default();
    let mut c = Camera::default();
    c.position = Vec3::new(0.0, 60.0, 260.0);
    c.target = Vec3::ZERO;

    let con = RasterStats::default();
    s.draw(&mut fb, &c, &nave, false, &RenderToggles::default(), &con);

    let sin = RasterStats::default();
    let toggles = RenderToggles { lod: false, ..Default::default() };
    s.draw(&mut fb, &c, &nave, false, &toggles, &sin);

    assert!(
        RasterStats::get(&con.triangles_submitted) < RasterStats::get(&sin.triangles_submitted),
        "los niveles de detalle deberian ahorrar triangulos de lejos"
    );
}

#[test]
fn el_recorrido_grabado_es_continuo() {
    let mut s = sistema();
    let tour = Tour::completo();
    assert!(tour.duracion() > 60.0, "el recorrido es muy corto");

    let dt = 1.0 / 30.0;
    let mut anterior = tour.posicion(&s, 0.0);
    let mut salto_maximo: f32 = 0.0;
    let pasos = (tour.duracion() / dt) as usize;

    for i in 1..pasos {
        s.update(dt);
        let p = tour.posicion(&s, i as f32 * dt);
        salto_maximo = salto_maximo.max((p - anterior).length());
        anterior = p;
    }
    // Sin cortes: en un treintavo de segundo la nave nunca da un salto
    // grande, ni siquiera al cambiar de planeta.
    assert!(salto_maximo < 6.0, "hubo un salto de {salto_maximo} unidades");
}

#[test]
fn el_recorrido_visita_todos_los_cuerpos() {
    let tour = Tour::completo();
    let s = sistema();
    let mut visitados: Vec<usize> = tour.tramos.iter().map(|t| t.cuerpo).collect();
    visitados.sort_unstable();
    visitados.dedup();
    assert_eq!(
        visitados.len(),
        s.bodies.len(),
        "el recorrido deberia pasar por los {} cuerpos",
        s.bodies.len()
    );
}

#[test]
fn una_nave_colocada_junto_a_un_planeta_no_cae_al_sol() {
    // Es lo que hace el programa al colocar la nave: le pasa la velocidad
    // orbital del cuerpo al que llega. Asi acompaña al planeta en su
    // vuelta alrededor de la estrella en vez de quedarse atras.
    let indice = 2;
    let mut s = sistema();
    let distancia = s.bodies[indice].outer_radius() * 2.2 + 0.6;

    let direccion = Vec3::new(1.0, 0.0, 0.0);
    let mut nave = Ship::default();
    nave.position = s.bodies[indice].position + direccion * distancia;
    let tangente = Vec3::UP.cross(direccion).normalized();
    nave.velocity =
        s.velocidad_de(indice) + tangente * s.velocidad_circular(indice, distancia);

    let inicial_al_sol = (nave.position - s.bodies[0].position).length();
    let dt = 1.0 / 60.0;
    let mut minima_al_sol = f32::INFINITY;
    let mut maxima_al_planeta: f32 = 0.0;
    for _ in 0..3600 {
        s.update(dt);
        s.aplicar_fisica(&mut nave, dt, true);
        nave.update(&ShipInput::default(), dt, None);
        minima_al_sol = minima_al_sol.min((nave.position - s.bodies[0].position).length());
        maxima_al_planeta =
            maxima_al_planeta.max((nave.position - s.bodies[indice].position).length());
    }

    assert!(
        minima_al_sol > inicial_al_sol * 0.7,
        "la nave se fue hacia la estrella: de {inicial_al_sol} a {minima_al_sol}"
    );
    // Y se queda dando vueltas al planeta en vez de soltarse o caer.
    assert!(
        maxima_al_planeta < distancia * 2.5,
        "la nave se solto del planeta: llego a {maxima_al_planeta}"
    );
    let final_al_planeta = (nave.position - s.bodies[indice].position).length();
    assert!(
        final_al_planeta > s.bodies[indice].outer_radius(),
        "la nave termino dentro del planeta: {final_al_planeta}"
    );
}

#[test]
fn una_nave_quieta_si_cae_hacia_la_estrella() {
    // El contraste de la prueba anterior: sin velocidad orbital, la
    // gravedad hace su trabajo y la nave cae. Y la colision la detiene en
    // la superficie en vez de dejarla atravesar la estrella.
    let mut s = sistema();
    let mut nave = Ship::default();
    nave.position = Vec3::new(40.0, 0.0, 0.0);
    nave.velocity = Vec3::ZERO;

    let inicial = nave.position.length();
    let dt = 1.0 / 120.0;
    let mut minima = f32::INFINITY;
    for _ in 0..3600 {
        s.update(dt);
        s.aplicar_fisica(&mut nave, dt, true);
        nave.update(&ShipInput::default(), dt, None);
        minima = minima.min((nave.position - s.bodies[0].position).length());
    }

    let final_ = (nave.position - s.bodies[0].position).length();
    assert!(final_ < inicial * 0.5, "deberia haber caido: de {inicial} a {final_}");
    assert!(
        minima >= s.bodies[0].radius,
        "la nave atraveso la estrella: llego a {minima}"
    );
}

#[test]
fn la_velocidad_orbital_es_la_derivada_de_la_posicion() {
    let s = sistema();
    let b = &s.bodies[2];
    let h = 0.05;
    let aproximada = (b.orbit.position(10.0 + h) - b.orbit.position(10.0 - h)) / (2.0 * h);
    let exacta = b.orbit.velocity(10.0);
    assert!(
        (aproximada - exacta).length() < exacta.length() * 0.01,
        "{:?} contra {:?}",
        aproximada,
        exacta
    );
}
