pub mod external_surface;
pub mod headless;
pub mod vulkan_interop;
pub mod vulkan_sharing;

use bevy::prelude::*;
use std::sync::Arc;
use thiserror::Error;

pub use external_surface::{ExternalSurface, ExternalSurfacePlugin, SurfaceTarget};
pub use headless::{HeadlessRenderPlugin, HeadlessRenderSettings};
pub use vulkan_interop::{ExternalMemoryHandle, VulkanExternalTexture};
pub use vulkan_sharing::{VulkanSharingPlugin, VulkanSharingConfig, SharedVulkanResources};

#[derive(Debug, Error)]
pub enum ExternalSurfaceError {
    #[error("Failed to create surface: {0}")]
    SurfaceCreationFailed(String),
    
    #[error("Vulkan extension not available: {0}")]
    VulkanExtensionNotAvailable(String),
    
    #[error("Failed to export memory: {0}")]
    MemoryExportFailed(String),
    
    #[error("Unsupported backend: {0}")]
    UnsupportedBackend(String),
    
    #[error("Invalid texture format")]
    InvalidTextureFormat,
}

pub type Result<T> = std::result::Result<T, ExternalSurfaceError>;

#[derive(Resource, Clone)]
pub struct ExternalRenderTarget {
    pub handle: Arc<dyn ExternalSurface>,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
}

#[derive(Component, Clone)]
pub struct RenderToExternal {
    pub target: Arc<dyn ExternalSurface>,
}