use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderDevice, RenderQueue},
        Extract, Render, RenderApp, RenderSet,
    },
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
use wgpu::{Surface, SurfaceConfiguration};

use crate::Result;

pub trait ExternalSurface: Send + Sync + 'static {
    fn as_image(&self) -> Option<Handle<Image>>;
    
    fn as_raw_texture(&self) -> Option<&wgpu::Texture>;
    
    fn present(&self) -> Result<()>;
    
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;
}

#[derive(Clone)]
pub enum SurfaceTarget {
    Window(Arc<dyn WindowHandle>),
    Texture(Handle<Image>),
    RawTexture(Arc<wgpu::Texture>),
}

impl std::fmt::Debug for SurfaceTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurfaceTarget::Window(_) => write!(f, "SurfaceTarget::Window"),
            SurfaceTarget::Texture(handle) => write!(f, "SurfaceTarget::Texture({:?})", handle),
            SurfaceTarget::RawTexture(_) => write!(f, "SurfaceTarget::RawTexture"),
        }
    }
}

pub trait WindowHandle: HasWindowHandle + HasDisplayHandle + Send + Sync {}

pub struct ExternalSurfacePlugin {
    pub target: SurfaceTarget,
    pub size: (u32, u32),
    pub format: TextureFormat,
}

impl Default for ExternalSurfacePlugin {
    fn default() -> Self {
        Self {
            target: SurfaceTarget::Texture(Handle::default()),
            size: (1920, 1080),
            format: TextureFormat::Bgra8UnormSrgb,
        }
    }
}

impl Plugin for ExternalSurfacePlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        
        render_app
            .add_systems(ExtractSchedule, extract_external_surfaces)
            .add_systems(
                Render,
                (
                    prepare_external_surfaces.in_set(RenderSet::PrepareResources),
                    render_to_external_surfaces.in_set(RenderSet::Render),
                ),
            );
    }
}

pub struct WindowSurface {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    size: (u32, u32),
}

impl WindowSurface {
    pub fn new(
        window: Arc<dyn WindowHandle>,
        device: &RenderDevice,
        size: (u32, u32),
        format: TextureFormat,
    ) -> Result<Self> {
        let instance = wgpu::Instance::default();
        
        // We need to use raw window and display handles directly
        let surface = unsafe {
            let raw_window = window.window_handle().map_err(|e| 
                crate::ExternalSurfaceError::SurfaceCreationFailed(e.to_string()))?;
            let raw_display = window.display_handle().map_err(|e| 
                crate::ExternalSurfaceError::SurfaceCreationFailed(e.to_string()))?;
            
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: raw_display.as_raw(),
                raw_window_handle: raw_window.as_raw(),
            };
            
            instance
                .create_surface_unsafe(target)
                .map_err(|e| crate::ExternalSurfaceError::SurfaceCreationFailed(e.to_string()))?
        };
        
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: format.into(),
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        
        surface.configure(device.wgpu_device(), &config);
        
        Ok(Self {
            surface,
            config,
            size,
        })
    }
}

impl ExternalSurface for WindowSurface {
    fn as_image(&self) -> Option<Handle<Image>> {
        None
    }
    
    fn as_raw_texture(&self) -> Option<&wgpu::Texture> {
        None
    }
    
    fn present(&self) -> Result<()> {
        Ok(())
    }
    
    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        Ok(())
    }
}

pub struct TextureSurface {
    image: Handle<Image>,
    texture: Option<Arc<wgpu::Texture>>,
    size: (u32, u32),
}

impl TextureSurface {
    pub fn new(
        images: &mut Assets<Image>,
        size: (u32, u32),
        format: TextureFormat,
    ) -> Self {
        let extent = Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        
        let mut image = Image::new_fill(
            extent,
            TextureDimension::D2,
            &[0, 0, 0, 0],
            format,
            RenderAssetUsages::RENDER_WORLD,
        );
        
        image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC;
        
        let handle = images.add(image);
        
        Self {
            image: handle,
            texture: None,
            size,
        }
    }
}

impl ExternalSurface for TextureSurface {
    fn as_image(&self) -> Option<Handle<Image>> {
        Some(self.image.clone())
    }
    
    fn as_raw_texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref().map(|t| t.as_ref())
    }
    
    fn present(&self) -> Result<()> {
        Ok(())
    }
    
    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.size = (width, height);
        Ok(())
    }
}

fn extract_external_surfaces(
    mut commands: Commands,
    surfaces: Extract<Query<(Entity, &crate::RenderToExternal)>>,
) {
    for (entity, render_to_external) in &surfaces {
        commands
            .entity(entity)
            .insert(render_to_external.clone());
    }
}

fn prepare_external_surfaces(
    mut surfaces: Query<&mut crate::RenderToExternal>,
    render_device: Res<RenderDevice>,
) {
}

fn render_to_external_surfaces(
    surfaces: Query<&crate::RenderToExternal>,
    render_queue: Res<RenderQueue>,
) {
    for surface in &surfaces {
        if let Err(e) = surface.target.present() {
            warn!("Failed to present external surface: {}", e);
        }
    }
}