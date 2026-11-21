//! Renderizador por software: framebuffer, pipeline y rasterizador.

pub mod framebuffer;
pub mod pipeline;
pub mod raster;

pub use framebuffer::Framebuffer;
pub use pipeline::{
    Blend, CullMode, DrawOptions, Fragment, FragmentShader, RasterStats, Uniforms,
};
