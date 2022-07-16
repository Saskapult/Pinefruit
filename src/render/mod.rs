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
pub use camera::*;

pub mod boundmaterial;
pub use boundmaterial::*;

pub mod boundmesh;
pub use boundmesh::*;

pub mod model;
pub use model::*;

pub mod shader;
pub use shader::*;

pub mod boundtexture;
pub use boundtexture::*;

pub mod vertex;
pub use vertex::*;

pub mod graph;
pub use graph::*;

pub mod light;
pub use light::*;

pub mod interpolation;
pub use interpolation::*;

pub mod rays;
pub use rays::*;

pub mod ssao;
pub use ssao::*;
