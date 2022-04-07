//! # This module is for rendering things.
//! 
//! This documentation is barely documentation, make it better.
//! 
//! The central thing, render instance, is cool.
//! 
//! ## The How
//! All shaders do not need to be known at any point in execution.
//! In order to accomplih this multiple vertex, instance, and material formats are used.
//! This will decrease performance if too many different formats are needed.
//! Please try to use the same formats even when that specific shader doesn't need it.
//! 
//! Please send help, I don't know what I'm doing



pub mod camera;
pub mod boundmaterial;
pub mod boundmesh;
pub mod model;
pub mod renderinstance;
pub mod shader;
pub mod boundtexture;
pub mod vertex;
pub mod graph;
pub mod light;
pub mod resources;
pub mod interpolation;
pub mod rays;

pub use camera::*;
pub use boundmaterial::*;
pub use boundmesh::*;
pub use model::*;
pub use renderinstance::*;
pub use shader::*;
pub use boundtexture::*;
pub use vertex::*;
pub use light::*;
pub use graph::*;
pub use resources::*;
