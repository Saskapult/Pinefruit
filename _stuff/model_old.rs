use crate::model::{
    texture::Texture,
    mesh::Mesh,
    material::Material,
    vertex::Vertex,
};
use std::ops::Range;
use std::path::Path;
use anyhow::*;
use tobj::LoadOptions;
use wgpu::util::DeviceExt;


// A model is composed of meshes with corresponding materials
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}
impl Model {
    pub fn load<P: AsRef<Path>>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        path: P,
    ) -> Result<Self> {
        // Use tobj to get models and materials
        let (obj_models, obj_materials) = tobj::load_obj(path.as_ref(), &LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
        )?;

        // Did the materials load correctly?
        let obj_materials = obj_materials?;

        // We're assuming that the texture files are stored with the obj file
        let containing_folder = path.as_ref().parent()
            .context("Directory has no parent")?;

        // Load all materials!
        let mut materials = Vec::new();
        for mat in obj_materials {
            let diffuse_path = mat.diffuse_texture;
            let diffuse_texture = Texture::load(device, queue, containing_folder.join(diffuse_path), false)?;

            // This should be optional in the future
            let normal_path = mat.normal_texture;
            let normal_texture = Texture::load(device, queue, containing_folder.join(normal_path), true)?;

            // Make a bind group for the two textures
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                    },
                ],
                label: None,
            });

            materials.push(Material {
                name: mat.name,
                diffuse_texture,
                normal_texture,
                bind_group,
            });
        }

        // Load all meshes!
        let mut meshes = Vec::new();
        for m in obj_models {
            let mut vertices = Vec::new();
            for i in 0..m.mesh.positions.len() / 3 {
                vertices.push(Vertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    colour: [1.0, 1.0, 1.0],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                });
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Vertex Buffer", path.as_ref())),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }
            );
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Index Buffer", path.as_ref())),
                    contents: bytemuck::cast_slice(&m.mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }
            );

            meshes.push(Mesh {
                name: m.name,
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            });
        }

        Ok(Self { meshes, materials })
    }
}


pub fn make_mesh_buffers(
    device: &wgpu::Device, 
    name: String, 
    vertices: Vec<Vertex>, 
    indices: Vec<u32>,
) -> (wgpu::Buffer, wgpu::Buffer) {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Vertex Buffer", name)),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }
    );
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{:?} Index Buffer", name)),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }
    );
    return (vertex_buffer, index_buffer)
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
impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
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
        model: &'b Model,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_model_instanced(model, 0..1, camera_bind_group, light_bind_group);
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group, light_bind_group);
        }
    }

}


fn create_cube_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let vertices = [
        [-1.0, -1.0, 1.0],   
        [1.0, -1.0, 1.0],    
        [1.0, 1.0, 1.0],     
        [-1.0, 1.0, 1.0],    
        [-1.0, 1.0, -1.0],   
        [1.0, 1.0, -1.0],    
        [1.0, -1.0, -1.0],   
        [-1.0, -1.0, -1.0],  
        [1.0, -1.0, -1.0],   
        [1.0, 1.0, -1.0],    
        [1.0, 1.0, 1.0],     
        [1.0, -1.0, 1.0],    
        [-1.0, -1.0, 1.0],   
        [-1.0, 1.0, 1.0],    
        [-1.0, 1.0, -1.0],   
        [-1.0, -1.0, -1.0],  
        [1.0, 1.0, -1.0],    
        [-1.0, 1.0, -1.0],   
        [-1.0, 1.0, 1.0],    
        [1.0, 1.0, 1.0],     
        [1.0, -1.0, 1.0],    
        [-1.0, -1.0, 1.0],   
        [-1.0, -1.0, -1.0],  
        [1.0, -1.0, -1.0],   
    ];

    let normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
    ];

    let tcs = [ 
        [1.0, 1.0],
        [0.0, 1.0], 
        [0.0, 0.0],
        [0.0, 0.0],
        [1.0, 0.0],   
        [1.0, 1.0],
        [0.0, 1.0],   
        [0.0, 0.0],  
        [1.0, 0.0],
        [1.0, 0.0],   
        [1.0, 1.0],   
        [0.0, 1.0],
        [1.0, 0.0],   
        [1.0, 1.0],  
        [0.0, 1.0],
        [0.0, 1.0],   
        [0.0, 0.0],  
        [1.0, 0.0],
        [1.0, 1.0],   
        [0.0, 1.0],  
        [0.0, 0.0],
        [0.0, 0.0],    
        [1.0, 0.0],   
        [1.0, 1.0],
        [0.0, 1.0],   
        [0.0, 0.0],   
        [1.0, 0.0],
        [1.0, 0.0],   
        [1.0, 1.0],    
        [0.0, 1.0],
        [0.0, 0.0], 
        [1.0, 0.0], 
        [1.0, 1.0],
        [1.0, 1.0], 
        [0.0, 1.0], 
        [0.0, 0.0],
    ];

    let mut vertex_data = Vec::new();
    for i in 0..vertices.len() {
        vertex_data.push(Vertex {
            position: vertices[i],
            colour: [1.0, 1.0, 1.0],
            tex_coords: tcs[i],
            normal: normals[i],
        });
    }

    let index_data: &[u16] = &[
        0, 1, 2, 2, 3, 0, // top
        4, 5, 6, 6, 7, 4, // bottom
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data, index_data.to_vec())
}