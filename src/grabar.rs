//! Grabacion fuera de pantalla.
//!
//! Renderiza el recorrido guionado cuadro por cuadro y entrega los pixeles
//! a `ffmpeg` por la entrada estandar. No hace falta grabar la pantalla:
//! el video sale del mismo renderizador que dibuja la ventana, a la
//! resolucion y a los cuadros por segundo que se pidan.
//!
//! Si `ffmpeg` no esta instalado, los cuadros se guardan como imagenes
//! PPM, que es un formato que se puede escribir sin ninguna dependencia.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::render::framebuffer::Framebuffer;
use crate::render::pipeline::RasterStats;
use crate::scene::camera::Camera;
use crate::scene::ship::Ship;
use crate::scene::system::{RenderToggles, SolarSystem};
use crate::scene::tour::Tour;

/// Convierte el framebuffer a bytes RGB.
fn a_rgb(fb: &Framebuffer, destino: &mut Vec<u8>) {
    destino.clear();
    destino.reserve(fb.color.len() * 3);
    for &c in &fb.color {
        destino.push(((c >> 16) & 0xFF) as u8);
        destino.push(((c >> 8) & 0xFF) as u8);
        destino.push((c & 0xFF) as u8);
    }
}

/// Guarda un solo cuadro como PPM.
pub fn captura_ppm(fb: &Framebuffer, ruta: &Path) -> std::io::Result<()> {
    let archivo = fs::File::create(ruta)?;
    let mut w = BufWriter::new(archivo);
    write!(w, "P6\n{} {}\n255\n", fb.width, fb.height)?;
    let mut bytes = Vec::new();
    a_rgb(fb, &mut bytes);
    w.write_all(&bytes)?;
    w.flush()
}

pub struct OpcionesGrabacion {
    pub ancho: usize,
    pub alto: usize,
    pub fps: u32,
    /// Duracion en segundos. Si es cero se usa la del recorrido.
    pub segundos: f32,
    /// Multiplicador del tiempo simulado.
    pub escala_tiempo: f32,
    pub salida: String,
}

impl Default for OpcionesGrabacion {
    fn default() -> Self {
        Self {
            ancho: 1280,
            alto: 720,
            fps: 30,
            segundos: 0.0,
            escala_tiempo: 1.0,
            salida: "docs/recorrido.mp4".to_string(),
        }
    }
}

/// Renderiza el recorrido completo y lo codifica en video.
pub fn grabar(semilla: u32, opciones: &OpcionesGrabacion) -> std::io::Result<()> {
    let mut sistema = SolarSystem::new(semilla);
    let tour = Tour::completo();
    let duracion = if opciones.segundos > 0.0 {
        opciones.segundos
    } else {
        tour.duracion()
    };
    let total = (duracion * opciones.fps as f32).round() as usize;

    if let Some(dir) = Path::new(&opciones.salida).parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }

    let mut hijo = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            &format!("{}x{}", opciones.ancho, opciones.alto),
            "-framerate",
            &opciones.fps.to_string(),
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "19",
            "-pix_fmt",
            "yuv420p",
            &opciones.salida,
        ])
        .stdin(Stdio::piped())
        .spawn()
        .ok();

    let carpeta_ppm = Path::new("docs/cuadros");
    if hijo.is_none() {
        eprintln!("ffmpeg no esta disponible: se guardaran cuadros PPM en {carpeta_ppm:?}");
        fs::create_dir_all(carpeta_ppm)?;
    }

    let mut fb = Framebuffer::new(opciones.ancho, opciones.alto);
    let mut nave = Ship::default();
    let mut camara = Camera::default();
    let toggles = RenderToggles::default();
    let stats = RasterStats::default();
    let mut bytes = Vec::new();

    let dt = 1.0 / opciones.fps as f32;
    let inicio = std::time::Instant::now();

    for cuadro in 0..total {
        let t = cuadro as f32 * dt;
        sistema.update(dt * opciones.escala_tiempo);
        tour.aplicar(&sistema, t, &mut nave, &mut camara);
        sistema.draw(&mut fb, &camara, &nave, true, &toggles, &stats);

        match hijo.as_mut() {
            Some(proc) => {
                a_rgb(&fb, &mut bytes);
                let stdin = proc.stdin.as_mut().expect("ffmpeg sin entrada");
                stdin.write_all(&bytes)?;
            }
            None => {
                captura_ppm(&fb, &carpeta_ppm.join(format!("cuadro{cuadro:05}.ppm")))?;
            }
        }

        if cuadro % opciones.fps as usize == 0 {
            let pct = cuadro as f32 / total as f32 * 100.0;
            let transcurrido = inicio.elapsed().as_secs_f32();
            eprintln!(
                "  {:5.1}%  cuadro {cuadro}/{total}  {:.1} s de computo",
                pct, transcurrido
            );
        }
    }

    if let Some(mut proc) = hijo {
        drop(proc.stdin.take());
        let estado = proc.wait()?;
        if estado.success() {
            println!("Video escrito en {}", opciones.salida);
        } else {
            eprintln!("ffmpeg termino con error");
        }
    }

    println!(
        "{total} cuadros en {:.1} s ({:.1} cuadros por segundo de computo)",
        inicio.elapsed().as_secs_f32(),
        total as f32 / inicio.elapsed().as_secs_f32()
    );
    Ok(())
}
