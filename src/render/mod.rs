//! Renderizador por software: framebuffer, pipeline, rasterizador y
//! texturas generadas en memoria.

pub mod framebuffer;
pub mod pipeline;
pub mod raster;
pub mod texture;

pub use framebuffer::Framebuffer;
pub use pipeline::{
    Blend, CullMode, DrawOptions, Fragment, FragmentShader, RasterStats, Uniforms,
};
