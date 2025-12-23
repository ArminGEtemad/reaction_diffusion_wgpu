use crate::gpu_resources::GpuResource;
use std::collections::HashMap;
use wgpu::*;

pub struct TextureResource {
    pub texture: Texture,
    pub view: TextureView,
}

pub struct ResourceRegistry {
    textures: HashMap<String, TextureResource>, // holds offscreen textures
    views: HashMap<String, TextureView>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            views: HashMap::new(),
        }
    }

    // get texture
    pub fn get_texture(&self, name: &str) -> Option<&TextureResource> {
        self.textures.get(name)
    }

    // if the texture exists with this name give me the view
    pub fn get_view(&self, name: &str) -> Option<&TextureView> {
        if let Some(texture) = self.textures.get(name) {
            return Some(&texture.view);
        }
        self.views.get(name)
    }

    // does it exist? if a texture with name doesn't exist create it now
    // this is used for display
    #[allow(dead_code)]
    pub fn color_texture_creator(&mut self, name: &str, gpu_res: &GpuResource) {
        if self.textures.contains_key(name) {
            return;
        }

        let size = gpu_res.size;
        let format = gpu_res.config.format;

        let texture = gpu_res.device.create_texture(&TextureDescriptor {
            label: Some(&format!("Register Texture Descriptor {}", name)),
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        self.textures
            .insert(name.to_string(), TextureResource { texture, view });
    }

    // creates the storeage texture for compute
    pub fn storage_texture_creator(
        &mut self,
        name: &str,
        gpu_res: &GpuResource,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) {
        if self.textures.contains_key(name) {
            return;
        }

        let texture = gpu_res.device.create_texture(&TextureDescriptor {
            label: Some(&format!("Register Texture Descriptor {}", name)),
            size: Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        self.textures
            .insert(name.to_string(), TextureResource { texture, view });
    }

    // get the view only without the tecture with the correct name
    pub fn set_view(&mut self, name: &str, view: &TextureView) {
        self.views.insert(name.to_string(), view.clone());
    }
}
