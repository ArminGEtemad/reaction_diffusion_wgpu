use crate::gpu_resources::GpuResource;
use std::collections::HashMap;
use wgpu::*;

pub struct TextureResource {
    pub texture: Texture,
    pub view: TextureView,
}

pub struct ResourceRegistry {
    offscreen_targets: HashMap<String, TextureResource>, // holds offscreen textures
    views: HashMap<String, TextureView>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            offscreen_targets: HashMap::new(),
            views: HashMap::new(),
        }
    }

    pub fn register_view(&self, name: &str) -> Option<&TextureView> {
        if let Some(texture) = self.offscreen_targets.get(name) {
            return Some(&texture.view);
        }
        self.views.get(name)
    }

    // does it exist?
    pub fn register_existence_target(&mut self, name: &str, gpu_res: &GpuResource) {
        if self.offscreen_targets.contains_key(name) {
            return;
        }

        let size = gpu_res.size;
        let format = gpu_res.config.format;

        let texture_register = gpu_res.device.create_texture(&TextureDescriptor {
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

        let view_register = texture_register.create_view(&TextureViewDescriptor::default());

        self.offscreen_targets.insert(
            name.to_string(),
            TextureResource {
                texture: texture_register,
                view: view_register,
            },
        );
    }

    pub fn set_view(&mut self, name: &str, view: &TextureView) {
        self.views.insert(name.to_string(), view.clone());
    }
}
