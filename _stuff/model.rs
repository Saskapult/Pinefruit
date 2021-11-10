use core::ops::Range;
use crate::model::{
	mesh::Mesh,
	material::Material,
	texture::*,
};
use std::path::Path;



#[derive(Debug)]
pub struct Model {
	pub meshes: Vec<&Mesh>,
	pub materials: Vec<&Material>,
}
impl Model {
	pub fn from_obj(
		path: &Path,
		device: &wgpu::Device,
        queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
	) -> Self {
		let (obj_models, obj_materials) = tobj::load_obj(
			path, 
			&tobj::LoadOptions {
				triangulate: true,
				single_index: true,
				..Default::default()
			},
		).expect("Failed to load OBJ file");

		let obj_materials = obj_materials.expect("Failed to load MTL file");
		// Generate default material?

		// Scan containing folder for stuff
		let containing_folder = path.parent().expect("Fugg in load obj");

		let mut mats = Vec::new();
		for mat in obj_materials {
			let dt = Texture::load(device, queue, &containing_folder.join(mat.diffuse_texture), TextureType::DiffuseTexture).expect("fuggy");
			let nt = Texture::load(device, queue, &containing_folder.join(mat.normal_texture), TextureType::NormalTexture).expect("fugg");
			mats.push(Material::new(
				mat.name,&dt,&nt, device, layout
			));
		}

		let mut mods = Vec::new();
		for m in obj_models {
			mods.push(Mesh::new(&m.name.to_string(), m, device));
		}

		Self {
			meshes: mods,
			materials: mats,
		}
	}
}


// This is what must be done to draw a model
pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}


// Teaches renderpass to draw a model
impl<'a> DrawModel<'a> for wgpu::RenderPass<'a>
{
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group, light_bind_group);
        }
    }

}


