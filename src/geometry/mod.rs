//! Generacion de geometria: mallas, primitivas, modelos propios,
//! esferas procedurales y campos de altura.

pub mod mesh;
pub mod models;
pub mod primitives;
pub mod sphere;
pub mod terrain;

pub use mesh::{Mesh, Vertex};
