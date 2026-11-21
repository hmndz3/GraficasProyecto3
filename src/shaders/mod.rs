//! Fragment shaders escritos para este proyecto.
//!
//! Cada uno resuelve el color de un pixel a partir de la posicion
//! interpolada, la normal, las coordenadas de textura y el tiempo. No hay
//! shaders de GPU: todo esto corre en CPU, dentro del rasterizador.

pub mod common;
pub mod gas;
pub mod ice;
pub mod ocean;
pub mod rings;
pub mod rocky;
pub mod solid;
pub mod star;

pub use gas::GasShader;
pub use ice::IceShader;
pub use ocean::OceanShader;
pub use rings::RingShader;
pub use rocky::RockyShader;
pub use solid::{AtmosphereShader, GlowShader, PropShader, ShipShader, TintShader};
pub use star::{CoronaShader, StarShader};
