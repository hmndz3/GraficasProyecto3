//! Texto de mapa de bits para el HUD.
//!
//! No hay libreria de fuentes: cada caracter es una rejilla de 5x7 pixeles
//! escrita a mano, y se dibuja directamente sobre el framebuffer despues
//! de rasterizar la escena.

use super::framebuffer::{pack_color, Framebuffer};
use crate::math::Vec3;

pub const ANCHO_GLIFO: usize = 5;
pub const ALTO_GLIFO: usize = 7;

/// Convierte siete cadenas de unos y ceros en las filas del glifo.
const fn g(filas: [u8; 7]) -> [u8; 7] {
    filas
}

/// Devuelve el glifo de un caracter. Lo que no esta en la tabla se dibuja
/// como espacio.
fn glifo(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => g([0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'C' => g([0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => g([0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
        'E' => g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => g([0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
        'H' => g([0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => g([0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'J' => g([0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
        'K' => g([0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => g([0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => g([0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => g([0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => g([0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => g([0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
        'R' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => g([0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
        'T' => g([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'W' => g([0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
        'X' => g([0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => g([0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '0' => g([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => g([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
        '3' => g([0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
        '4' => g([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => g([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => g([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => g([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => g([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
        '.' => g([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100]),
        ',' => g([0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000]),
        ':' => g([0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000]),
        '-' => g([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '_' => g([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
        '/' => g([0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000]),
        '+' => g([0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
        '%' => g([0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011]),
        '(' => g([0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
        ')' => g([0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
        '[' => g([0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]),
        ']' => g([0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]),
        '<' => g([0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
        '>' => g([0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
        '!' => g([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
        '?' => g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
        '=' => g([0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]),
        '*' => g([0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000]),
        '#' => g([0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010]),
        _ => [0; 7],
    }
}

/// Ancho en pixeles que ocupara una cadena.
pub fn ancho_texto(texto: &str, escala: usize) -> usize {
    texto.chars().count() * (ANCHO_GLIFO + 1) * escala
}

/// Dibuja una cadena. La posicion es la esquina superior izquierda.
pub fn draw_text(fb: &mut Framebuffer, x: i64, y: i64, texto: &str, color: Vec3, escala: usize) {
    let escala = escala.max(1) as i64;
    let empacado = pack_color(color);
    let mut cursor = x;

    for c in texto.chars() {
        let bits = glifo(c);
        for (fila, bits_fila) in bits.iter().enumerate() {
            for col in 0..ANCHO_GLIFO {
                // El bit mas significativo de los cinco es la columna de
                // la izquierda.
                if bits_fila & (1 << (ANCHO_GLIFO - 1 - col)) == 0 {
                    continue;
                }
                for dy in 0..escala {
                    for dx in 0..escala {
                        let px = cursor + col as i64 * escala + dx;
                        let py = y + fila as i64 * escala + dy;
                        if px < 0 || py < 0 || px >= fb.width as i64 || py >= fb.height as i64 {
                            continue;
                        }
                        fb.color[py as usize * fb.width + px as usize] = empacado;
                    }
                }
            }
        }
        cursor += (ANCHO_GLIFO as i64 + 1) * escala;
    }
}

/// Oscurece un rectangulo para que el texto del HUD se lea sobre
/// cualquier fondo.
pub fn panel(fb: &mut Framebuffer, x: i64, y: i64, ancho: i64, alto: i64, opacidad: f32) {
    let k = (1.0 - opacidad.clamp(0.0, 1.0) * 0.82).clamp(0.0, 1.0);
    for py in y.max(0)..(y + alto).min(fb.height as i64) {
        for px in x.max(0)..(x + ancho).min(fb.width as i64) {
            let i = py as usize * fb.width + px as usize;
            let c = fb.color[i];
            let r = (((c >> 16) & 0xFF) as f32 * k) as u32;
            let gr = (((c >> 8) & 0xFF) as f32 * k) as u32;
            let b = ((c & 0xFF) as f32 * k) as u32;
            fb.color[i] = (r << 16) | (gr << 8) | b;
        }
    }
}

/// Dibuja una barra horizontal con relleno proporcional a `valor`.
pub fn barra(
    fb: &mut Framebuffer,
    x: i64,
    y: i64,
    ancho: i64,
    alto: i64,
    valor: f32,
    color: Vec3,
) {
    let lleno = (ancho as f32 * valor.clamp(0.0, 1.0)) as i64;
    let borde = pack_color(color * 0.35);
    let relleno = pack_color(color);
    for py in y.max(0)..(y + alto).min(fb.height as i64) {
        for px in x.max(0)..(x + ancho).min(fb.width as i64) {
            let i = py as usize * fb.width + px as usize;
            fb.color[i] = if px - x < lleno { relleno } else { borde };
        }
    }
}
