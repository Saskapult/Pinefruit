use crate::model::texture::Texture;


// A material is a texture and normal map combo
#[derive(Debug)]
pub struct Material {
    pub name: String,
    // diffuse_texture: Texture,
    // normal_texture: Texture,
    // pub ka: f32,                        // Ambient colour
    // pub kd: f32,                        // Diffuse colour
    // pub ks: f32,                        // Specular colour
    // pub ns: f32,                        // Specular exponent
    // pub d: f32,                         // Transparency
    // pub ni: f32,                        // Index of refraction
    // pub illum: i32,                     // Illumination model to use (see wikipedia)
    pub bind_group: wgpu::BindGroup,
}
impl Material {
    pub fn new(
        name: String, 
        diffuse_texture: &Texture, 
        normal_texture: &Texture,
        device: &wgpu::Device,
		layout: &wgpu::BindGroupLayout,
    ) -> Self {
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
            label: Some(&name),
        });

        Self {
            name,
            // diffuse_texture,
            // normal_texture,
            bind_group,
        }
    }
}




