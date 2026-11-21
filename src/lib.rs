//! Simulacion de un sistema solar dibujada con un renderizador por
//! software escrito desde cero.
//!
//! La libreria de ventanas solo abre la ventana, entrega el teclado y
//! muestra el framebuffer. Todo lo demas -transformaciones, recorte,
//! rasterizado, profundidad, shaders, geometria y texturas- vive en estos
//! modulos.

pub mod geometry;
pub mod math;
pub mod noise;
pub mod render;
pub mod shaders;
