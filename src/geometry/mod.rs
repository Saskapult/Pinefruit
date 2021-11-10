pub mod vertex;
pub mod mesh;
pub mod texture;
pub mod material;
//pub mod loader;

// Allows for "use model::Vertex;"
pub use self::vertex::*;
pub use self::mesh::*;
pub use self::texture::*;
pub use self::material::*;

