//! Programa principal: abre la ventana, lee el teclado, simula y dibuja.
//!
//! La libreria de ventanas solo hace tres cosas: crear la ventana,
//! reportar que teclas estan presionadas y mostrar en pantalla el bloque
//! de pixeles que produjo el renderizador. No se usa ninguna de sus
//! funciones de dibujo.

use std::time::Instant;

use minifb::{Key, KeyRepeat, Window, WindowOptions};

use sistema_solar::grabar::{grabar, OpcionesGrabacion};
use sistema_solar::math::Vec3;
use sistema_solar::render::framebuffer::Framebuffer;
use sistema_solar::render::pipeline::RasterStats;
use sistema_solar::render::text::{ancho_texto, barra, draw_text, panel};
use sistema_solar::scene::camera::{Camera, CameraMode};
use sistema_solar::scene::ship::{FlightMode, Ship, ShipInput};
use sistema_solar::scene::system::{RenderToggles, SolarSystem};

const ANCHO_VENTANA: usize = 1280;
const ALTO_VENTANA: usize = 720;

/// Semilla del sistema. Cambiarla genera otro sistema solar completo:
/// otro relieve, otras nebulosas y otra distribucion de objetos.
const SEMILLA: u32 = 20_260_413;

struct Estado {
    camara: Camera,
    nave: Ship,
    objetivo: usize,
    toggles: RenderToggles,
    gravedad: bool,
    hud: bool,
    pausa: bool,
    escala_tiempo: f32,
    escala_render: f32,
    choque: f32,
}

fn main() {
    // Modo de grabacion: renderiza el recorrido guionado sin abrir
    // ventana y lo entrega a ffmpeg.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|a| a.as_str()) == Some("grabar") {
        let mut op = OpcionesGrabacion::default();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--salida" => { op.salida = args[i + 1].clone(); i += 2; }
                "--ancho" => { op.ancho = args[i + 1].parse().unwrap_or(op.ancho); i += 2; }
                "--alto" => { op.alto = args[i + 1].parse().unwrap_or(op.alto); i += 2; }
                "--fps" => { op.fps = args[i + 1].parse().unwrap_or(op.fps); i += 2; }
                "--segundos" => { op.segundos = args[i + 1].parse().unwrap_or(0.0); i += 2; }
                "--tiempo" => { op.escala_tiempo = args[i + 1].parse().unwrap_or(1.0); i += 2; }
                otro => { eprintln!("opcion desconocida: {otro}"); i += 1; }
            }
        }
        println!(
            "Grabando {}x{} a {} fps en {}",
            op.ancho, op.alto, op.fps, op.salida
        );
        if let Err(e) = grabar(SEMILLA, &op) {
            eprintln!("error al grabar: {e}");
            std::process::exit(1);
        }
        return;
    }

    println!("Generando el sistema solar...");
    let inicio = Instant::now();
    let mut sistema = SolarSystem::new(SEMILLA);
    sistema.update(0.0);
    println!(
        "Listo en {:.2} s: {} cuerpos, {} triangulos en el nivel mas alto.",
        inicio.elapsed().as_secs_f32(),
        sistema.bodies.len(),
        sistema
            .bodies
            .iter()
            .map(|b| b.lods[0].triangle_count())
            .sum::<usize>()
    );

    let mut window = Window::new(
        "Sistema solar - renderizador por software",
        ANCHO_VENTANA,
        ALTO_VENTANA,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )
    .expect("no se pudo abrir la ventana");
    window.set_target_fps(0);

    let mut estado = Estado {
        camara: Camera::default(),
        nave: Ship::default(),
        objetivo: 2,
        toggles: RenderToggles::default(),
        gravedad: true,
        hud: true,
        pausa: false,
        escala_tiempo: 1.0,
        escala_render: 0.8,
        choque: 0.0,
    };

    // La nave arranca cerca de TERRANOVA para que la demostracion empiece
    // con algo a la vista.
    colocar_junto_a(&mut estado, &sistema, 2);

    let mut fb = Framebuffer::new(ANCHO_VENTANA, ALTO_VENTANA);
    let stats = RasterStats::default();
    let mut ultimo = Instant::now();
    let mut fps_suave = 60.0f32;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let ahora = Instant::now();
        let dt_real = (ahora - ultimo).as_secs_f32().min(0.1);
        ultimo = ahora;
        if dt_real > 0.0 {
            fps_suave = fps_suave * 0.9 + (1.0 / dt_real) * 0.1;
        }

        let (vw, vh) = window.get_size();
        let rw = ((vw as f32 * estado.escala_render) as usize).max(160);
        let rh = ((vh as f32 * estado.escala_render) as usize).max(90);
        fb.resize(rw, rh);

        leer_teclas(&window, &mut estado, &sistema);

        let dt = if estado.pausa { 0.0 } else { dt_real * estado.escala_tiempo };
        sistema.update(dt);
        actualizar(&mut estado, &sistema, &window, dt, dt_real);

        let mostrar_nave = estado.camara.mode != CameraMode::Cabina;
        sistema.draw(
            &mut fb,
            &estado.camara,
            &estado.nave,
            mostrar_nave,
            &estado.toggles,
            &stats,
        );

        if estado.hud {
            dibujar_hud(&mut fb, &estado, &sistema, &stats, fps_suave);
        }

        window
            .update_with_buffer(&fb.color, fb.width, fb.height)
            .expect("no se pudo presentar el framebuffer");
    }
}

/// Deja la nave a una distancia comoda del cuerpo indicado.
fn colocar_junto_a(estado: &mut Estado, sistema: &SolarSystem, indice: usize) {
    let b = &sistema.bodies[indice];
    let distancia = b.outer_radius() * 2.2 + 0.6;
    // Se llega desde el lado iluminado para que el planeta se vea.
    let hacia_el_sol = (sistema.bodies[0].position - b.position).normalized();
    let lateral = hacia_el_sol.cross(Vec3::UP).normalized();
    let direccion = (hacia_el_sol * 0.75 + lateral * 0.6 + Vec3::UP * 0.25).normalized();

    estado.nave.position = b.position + direccion * distancia;
    estado.nave.throttle = 0.0;

    // La nave hereda la velocidad orbital del cuerpo al que llega. Si se
    // quedara en reposo respecto de la estrella, la gravedad la arrastraria
    // hacia ella mientras el planeta sigue su camino.
    estado.nave.velocity = sistema.velocidad_de(indice);
    estado.nave.forward = (b.position - estado.nave.position).normalized();
    estado.nave.up = Vec3::UP;
    estado.nave.right = estado.nave.up.cross(estado.nave.forward).normalized();
    estado.nave.up = estado.nave.forward.cross(estado.nave.right).normalized();

    estado.objetivo = indice;
    estado.camara.position = estado.nave.position - estado.nave.forward * 8.0 + Vec3::UP * 2.5;
    estado.camara.target = b.position;
}

fn leer_teclas(window: &Window, estado: &mut Estado, sistema: &SolarSystem) {
    if window.is_key_pressed(Key::C, KeyRepeat::No) {
        estado.camara.mode = estado.camara.mode.siguiente();
    }
    if window.is_key_pressed(Key::L, KeyRepeat::No) {
        estado.nave.mode = match estado.nave.mode {
            FlightMode::Crucero => FlightMode::Local,
            FlightMode::Local => FlightMode::Crucero,
        };
    }
    if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
        estado.objetivo = (estado.objetivo + 1) % sistema.bodies.len();
        if estado.objetivo == 0 {
            estado.objetivo = 1;
        }
    }
    if window.is_key_pressed(Key::R, KeyRepeat::No) {
        let destino = estado.objetivo;
        colocar_junto_a(estado, sistema, destino);
    }
    if window.is_key_pressed(Key::G, KeyRepeat::No) {
        estado.gravedad = !estado.gravedad;
    }
    if window.is_key_pressed(Key::F, KeyRepeat::No) {
        estado.toggles.frustum_culling = !estado.toggles.frustum_culling;
    }
    if window.is_key_pressed(Key::O, KeyRepeat::No) {
        estado.toggles.lod = !estado.toggles.lod;
    }
    if window.is_key_pressed(Key::P, KeyRepeat::No) {
        estado.toggles.props = !estado.toggles.props;
    }
    if window.is_key_pressed(Key::B, KeyRepeat::No) {
        estado.toggles.orbitas = !estado.toggles.orbitas;
    }
    if window.is_key_pressed(Key::N, KeyRepeat::No) {
        estado.toggles.atmosferas = !estado.toggles.atmosferas;
    }
    if window.is_key_pressed(Key::H, KeyRepeat::No) {
        estado.hud = !estado.hud;
    }
    if window.is_key_pressed(Key::Space, KeyRepeat::No) && window.is_key_down(Key::LeftCtrl) {
        estado.pausa = !estado.pausa;
    }
    if window.is_key_pressed(Key::Period, KeyRepeat::Yes) {
        estado.escala_tiempo = (estado.escala_tiempo * 1.4).min(40.0);
    }
    if window.is_key_pressed(Key::Comma, KeyRepeat::Yes) {
        estado.escala_tiempo = (estado.escala_tiempo / 1.4).max(0.05);
    }
    if window.is_key_pressed(Key::Key9, KeyRepeat::No) {
        estado.escala_render = (estado.escala_render - 0.1).max(0.3);
    }
    if window.is_key_pressed(Key::Key0, KeyRepeat::No) {
        estado.escala_render = (estado.escala_render + 0.1).min(1.0);
    }
}

fn eje(window: &Window, positiva: Key, negativa: Key) -> f32 {
    let mut v = 0.0;
    if window.is_key_down(positiva) {
        v += 1.0;
    }
    if window.is_key_down(negativa) {
        v -= 1.0;
    }
    v
}

fn actualizar(
    estado: &mut Estado,
    sistema: &SolarSystem,
    window: &Window,
    dt: f32,
    dt_real: f32,
) {
    estado.choque = (estado.choque - dt_real * 2.0).max(0.0);

    match estado.camara.mode {
        CameraMode::Libre => {
            // En modo libre la camara vuela sola y la nave queda quieta.
            let velocidad = if window.is_key_down(Key::LeftShift) { 95.0 } else { 26.0 };
            let giro = 1.6 * dt_real;
            estado.camara.free_yaw += eje(window, Key::D, Key::A) * giro;
            estado.camara.free_pitch = (estado.camara.free_pitch
                + eje(window, Key::Up, Key::Down) * giro)
                .clamp(-1.45, 1.45);
            let avance = eje(window, Key::W, Key::S);
            let lateral = eje(window, Key::Right, Key::Left);
            let vertical = eje(window, Key::X, Key::Z);
            estado
                .camara
                .free_move(avance, lateral, vertical, velocidad, dt_real);
        }
        CameraMode::Orbital => {
            // Giro alrededor del cuerpo seleccionado, sin cortar la
            // simulacion.
            let giro = 1.3 * dt_real;
            estado.camara.orbit_yaw += eje(window, Key::D, Key::A) * giro;
            estado.camara.orbit_pitch = (estado.camara.orbit_pitch
                + eje(window, Key::Up, Key::Down) * giro)
                .clamp(-1.35, 1.35);
            let zoom = eje(window, Key::S, Key::W) * dt_real * 14.0;
            let b = &sistema.bodies[estado.objetivo];
            estado.camara.orbit_distance =
                (estado.camara.orbit_distance + zoom).clamp(b.outer_radius() * 1.5, 420.0);
            estado
                .camara
                .orbit(b.position, b.outer_radius(), dt_real);
        }
        _ => {
            // Modos atados a la nave: el jugador la pilotea.
            let input = ShipInput {
                thrust: eje(window, Key::W, Key::S),
                yaw: eje(window, Key::D, Key::A),
                pitch: eje(window, Key::Down, Key::Up),
                roll: eje(window, Key::E, Key::Q),
                strafe: eje(window, Key::Right, Key::Left),
                lift: eje(window, Key::X, Key::Z),
                boost: window.is_key_down(Key::LeftShift),
                brake: window.is_key_down(Key::Space) && !window.is_key_down(Key::LeftCtrl),
            };

            // En modo local, la vertical del planeta mas cercano sirve de
            // referencia para nivelar la nave.
            let cercano = sistema.cuerpo_mas_cercano(estado.nave.position);
            let vertical = if estado.nave.mode == FlightMode::Local {
                let d = estado.nave.position - sistema.bodies[cercano].position;
                if d.length() > 1e-4 {
                    Some(d.normalized())
                } else {
                    None
                }
            } else {
                None
            };

            estado.nave.update(&input, dt_real, vertical);
            if sistema.aplicar_fisica(&mut estado.nave, dt.max(dt_real * 0.0), estado.gravedad) {
                estado.choque = 1.0;
            }

            match estado.camara.mode {
                CameraMode::Cabina => estado.camara.cockpit(&estado.nave),
                _ => estado.camara.follow(&estado.nave, dt_real),
            }
        }
    }
}

fn dibujar_hud(
    fb: &mut Framebuffer,
    estado: &Estado,
    sistema: &SolarSystem,
    stats: &RasterStats,
    fps: f32,
) {
    let escala = if fb.width >= 1100 { 2 } else { 1 };
    let alto_linea = 10 * escala as i64;
    let margen = 8i64;

    let objetivo = &sistema.bodies[estado.objetivo];
    let distancia = (estado.nave.position - objetivo.position).length() - objetivo.outer_radius();
    let altura = sistema.altura_sobre(estado.objetivo, estado.nave.position);
    let velocidad = estado.nave.velocity.length();

    let enviados = RasterStats::get(&stats.triangles_submitted);
    let dibujados = RasterStats::get(&stats.triangles_rasterized);
    let traseros = RasterStats::get(&stats.triangles_backface);
    let fuera = RasterStats::get(&stats.triangles_offscreen);
    let fragmentos = RasterStats::get(&stats.fragments_shaded);
    let mallas_fuera = RasterStats::get(&stats.meshes_culled);

    let lineas = vec![
        format!(
            "SISTEMA HELIOS   {:.0} FPS   {}X{}   T X{:.2}{}",
            fps,
            fb.width,
            fb.height,
            estado.escala_tiempo,
            if estado.pausa { "   PAUSA" } else { "" }
        ),
        format!(
            "CAMARA {}   VUELO {}   GRAVEDAD {}",
            estado.camara.mode.nombre(),
            if estado.nave.mode == FlightMode::Local { "LOCAL" } else { "CRUCERO" },
            si_no(estado.gravedad)
        ),
        format!(
            "OBJETIVO {}   DIST {:.1}   ALTURA {:.1}",
            objetivo.name, distancia, altura
        ),
        format!(
            "VELOCIDAD {:.1} U/S   ACELERADOR {:+.0}%",
            velocidad,
            estado.nave.throttle * 100.0
        ),
        format!(
            "TRIS ENVIADOS {}   RASTERIZADOS {}   TRASEROS {}   FUERA {}",
            enviados, dibujados, traseros, fuera
        ),
        format!(
            "FRAGMENTOS {}   MALLAS DESCARTADAS {}   CULLING {}   LOD {}",
            fragmentos,
            mallas_fuera,
            si_no(estado.toggles.frustum_culling),
            si_no(estado.toggles.lod)
        ),
    ];

    let ancho = lineas
        .iter()
        .map(|l| ancho_texto(l, escala))
        .max()
        .unwrap_or(0) as i64;

    panel(
        fb,
        0,
        0,
        ancho + margen * 2,
        alto_linea * lineas.len() as i64 + margen * 2,
        0.8,
    );

    let color = Vec3::new(0.75, 0.92, 1.0);
    for (i, l) in lineas.iter().enumerate() {
        draw_text(
            fb,
            margen,
            margen + i as i64 * alto_linea,
            l,
            color,
            escala,
        );
    }

    // Barra del acelerador abajo a la izquierda.
    let y = fb.height as i64 - 26 * escala as i64;
    draw_text(fb, margen, y, "EMPUJE", color, escala);
    barra(
        fb,
        margen + ancho_texto("EMPUJE ", escala) as i64,
        y,
        120 * escala as i64,
        7 * escala as i64,
        estado.nave.throttle.abs(),
        if estado.nave.throttle >= 0.0 {
            Vec3::new(0.35, 0.85, 1.0)
        } else {
            Vec3::new(1.0, 0.55, 0.25)
        },
    );

    // Ayuda de controles abajo a la derecha.
    let ayuda = [
        "W/S EMPUJE   A/D GIRO   FLECHAS CABECEO Y LATERAL   Q/E ALABEO",
        "SHIFT TURBO   ESPACIO FRENO   C CAMARA   L MODO LOCAL   TAB OBJETIVO   R IR",
        "F CULLING   O LOD   P OBJETOS   B ORBITAS   N ATMOSFERAS   G GRAVEDAD   H HUD",
    ];
    let ancho_ayuda = ayuda
        .iter()
        .map(|l| ancho_texto(l, 1))
        .max()
        .unwrap_or(0) as i64;
    let x0 = fb.width as i64 - ancho_ayuda - margen * 2;
    let y0 = fb.height as i64 - 10 * ayuda.len() as i64 - margen * 2;
    panel(fb, x0, y0, ancho_ayuda + margen * 2, 10 * ayuda.len() as i64 + margen * 2, 0.7);
    for (i, l) in ayuda.iter().enumerate() {
        draw_text(
            fb,
            x0 + margen,
            y0 + margen + i as i64 * 10,
            l,
            Vec3::new(0.62, 0.70, 0.80),
            1,
        );
    }

    // Aviso de colision.
    if estado.choque > 0.0 {
        let texto = "IMPACTO";
        let w = ancho_texto(texto, escala + 1) as i64;
        draw_text(
            fb,
            (fb.width as i64 - w) / 2,
            fb.height as i64 / 4,
            texto,
            Vec3::new(1.0, 0.35, 0.2) * estado.choque,
            escala + 1,
        );
    }

    // Mira en el centro, solo en los modos de vuelo.
    if estado.camara.mode == CameraMode::Cabina || estado.camara.mode == CameraMode::Persecucion {
        let cx = fb.width as i64 / 2;
        let cy = fb.height as i64 / 2;
        draw_text(fb, cx - 2, cy - 3, "+", Vec3::new(0.5, 0.9, 0.7), 1);
    }

}

fn si_no(v: bool) -> &'static str {
    if v {
        "ON"
    } else {
        "OFF"
    }
}
