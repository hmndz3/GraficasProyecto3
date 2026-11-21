//! Framebuffer de color y buffer de profundidad.
//!
//! El color se guarda ya convertido a `0RGB` de 32 bits porque es el
//! formato que espera la ventana. La profundidad se guarda por separado en
//! coma flotante para la prueba de z.

use crate::math::Vec3;

/// Valor inicial del z-buffer: el plano lejano.
pub const DEPTH_CLEAR: f32 = f32::INFINITY;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub color: Vec<u32>,
    pub depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![0; width * height],
            depth: vec![DEPTH_CLEAR; width * height],
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        self.color = vec![0; width * height];
        self.depth = vec![DEPTH_CLEAR; width * height];
    }

    pub fn clear(&mut self, color: u32) {
        self.color.fill(color);
        self.depth.fill(DEPTH_CLEAR);
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Divide el framebuffer en bandas horizontales para rasterizar en
    /// paralelo. Cada banda es dueña de sus filas, asi que no hace falta
    /// sincronizar la escritura de pixeles.
    pub fn bands(&mut self, count: usize) -> Vec<Band<'_>> {
        let count = count.max(1);
        let rows = (self.height + count - 1) / count;
        let rows = rows.max(1);
        let width = self.width;
        let height = self.height;

        let mut color_rest: &mut [u32] = &mut self.color;
        let mut depth_rest: &mut [f32] = &mut self.depth;
        let mut bands = Vec::new();
        let mut y0 = 0usize;

        while y0 < height {
            let y1 = (y0 + rows).min(height);
            let take = (y1 - y0) * width;
            let (c, cr) = color_rest.split_at_mut(take);
            let (d, dr) = depth_rest.split_at_mut(take);
            color_rest = cr;
            depth_rest = dr;
            bands.push(Band { y0, y1, width, color: c, depth: d });
            y0 = y1;
        }
        bands
    }
}

/// Un bloque de filas del framebuffer sobre el que trabaja un solo hilo.
pub struct Band<'a> {
    pub y0: usize,
    pub y1: usize,
    pub width: usize,
    pub color: &'a mut [u32],
    pub depth: &'a mut [f32],
}

impl Band<'_> {
    #[inline(always)]
    pub fn index(&self, x: usize, y: usize) -> usize {
        (y - self.y0) * self.width + x
    }
}

/// Convierte un color lineal en el entero que consume la ventana.
///
/// Se aplica un mapeo de tono de Reinhard para que el nucleo del sol y los
/// reflejos especulares no se recorten a blanco plano, y despues la
/// correccion gamma estandar.
#[inline]
pub fn pack_color(c: Vec3) -> u32 {
    let mapped = Vec3::new(c.x / (1.0 + c.x), c.y / (1.0 + c.y), c.z / (1.0 + c.z));
    let g = Vec3::new(
        mapped.x.max(0.0).powf(1.0 / 2.2),
        mapped.y.max(0.0).powf(1.0 / 2.2),
        mapped.z.max(0.0).powf(1.0 / 2.2),
    );
    let r = (g.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let gr = (g.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let b = (g.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    (r << 16) | (gr << 8) | b
}
