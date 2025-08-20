use bevy::{
    prelude::*,
    log::{info, warn, error},
    render::{
        camera::{RenderTarget, ManualTextureView, ManualTextureViewHandle, ManualTextureViews},
        renderer::RenderDevice,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        Render, RenderApp, RenderSet, ExtractSchedule,
    },
};
use ash::{self, vk};
use std::sync::{Arc, Mutex};
use wgpu_hal::api::Vulkan as VulkanApi;

#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(unix)]
use nix::sys::socket::{self, ControlMessageOwned, MsgFlags, UnixAddr};

use crate::{ExternalSurfaceError, Result};

#[derive(Debug, Clone)]
pub struct VulkanSharingConfig {
    pub width: u32,
    pub height: u32,
    pub format: vk::Format,
    pub ipc_socket_path: Option<String>,
    pub enable_double_buffering: bool,
}

impl Default for VulkanSharingConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            format: vk::Format::B8G8R8A8_SRGB,
            ipc_socket_path: Some("/tmp/bevy_vulkan_sharing.sock".to_string()),
            enable_double_buffering: true,
        }
    }
}

#[derive(Resource, Clone)]
pub struct SharedVulkanResources {
    pub config: VulkanSharingConfig,
    pub texture_handles: Vec<ManualTextureViewHandle>,
    pub vulkan_images: Vec<vk::Image>,
    pub vulkan_memory: Vec<vk::DeviceMemory>,
    pub memory_fds: Vec<RawFd>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub consumer_ready_semaphores: Vec<vk::Semaphore>,
    pub current_buffer_index: usize,
    pub ipc_handler: Option<Arc<Mutex<IPCHandler>>>,
    // Store device handles for cleanup
    device: Option<Arc<ash::Device>>,
}

impl SharedVulkanResources {
    fn new(config: VulkanSharingConfig) -> Self {
        Self {
            config,
            texture_handles: Vec::new(),
            vulkan_images: Vec::new(),
            vulkan_memory: Vec::new(),
            memory_fds: Vec::new(),
            render_finished_semaphores: Vec::new(),
            consumer_ready_semaphores: Vec::new(),
            current_buffer_index: 0,
            ipc_handler: None,
            device: None,
        }
    }
    
    pub fn get_current_texture_handle(&self) -> Option<ManualTextureViewHandle> {
        self.texture_handles.get(self.current_buffer_index).copied()
    }
    
    pub fn swap_buffers(&mut self) {
        if self.config.enable_double_buffering && self.texture_handles.len() > 1 {
            self.current_buffer_index = (self.current_buffer_index + 1) % self.texture_handles.len();
        }
    }
}

// Make it extractable
impl ExtractResource for SharedVulkanResources {
    type Source = SharedVulkanResources;
    
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

// Cleanup on drop
impl Drop for SharedVulkanResources {
    fn drop(&mut self) {
        if let Some(device) = &self.device {
            unsafe {
                for &semaphore in &self.render_finished_semaphores {
                    device.destroy_semaphore(semaphore, None);
                }
                for &semaphore in &self.consumer_ready_semaphores {
                    device.destroy_semaphore(semaphore, None);
                }
                for &memory in &self.vulkan_memory {
                    device.free_memory(memory, None);
                }
                for &image in &self.vulkan_images {
                    device.destroy_image(image, None);
                }
            }
        }
    }
}

pub struct VulkanSharingPlugin {
    pub config: VulkanSharingConfig,
}

impl Default for VulkanSharingPlugin {
    fn default() -> Self {
        Self {
            config: VulkanSharingConfig::default(),
        }
    }
}

impl Plugin for VulkanSharingPlugin {
    fn build(&self, app: &mut App) {
        let resources = SharedVulkanResources::new(self.config.clone());
        
        app.insert_resource(resources.clone());
        app.add_plugins(ExtractResourcePlugin::<SharedVulkanResources>::default());
        
        // Add system to update camera targets in MainWorld
        app.add_systems(ExtractSchedule, extract_shared_resources);
        
        let render_app = app.sub_app_mut(RenderApp);
        
        render_app.insert_resource(resources);
        
        // Setup will run in RenderApp's Startup after RenderDevice is available
        render_app.add_systems(
            bevy::app::Startup,
            setup_vulkan_sharing,
        );
        
        render_app.add_systems(
            Render,
            (
                wait_for_consumer.in_set(RenderSet::PrepareResources),
                signal_render_finished.in_set(RenderSet::Cleanup),
            ),
        );
    }
}

fn extract_shared_resources(
    shared_resources: Res<SharedVulkanResources>,
    mut cameras: Query<&mut Camera>,
) {
    // Update camera targets to point to our shared texture
    if let Some(handle) = shared_resources.get_current_texture_handle() {
        for mut camera in cameras.iter_mut() {
            camera.target = RenderTarget::TextureView(handle);
        }
    }
}

fn setup_vulkan_sharing(
    render_device: Res<RenderDevice>,
    mut manual_texture_views: ResMut<ManualTextureViews>,
    mut shared_resources: ResMut<SharedVulkanResources>,
) {
    info!("Setting up Vulkan sharing with config: {:?}", shared_resources.config);
    
    let wgpu_device = render_device.wgpu_device();
    
    // Access the HAL device to get raw Vulkan handles
    let setup_result = unsafe {
        wgpu_device.as_hal::<VulkanApi, _, Result<()>>(|hal_device| {
            let hal_device = hal_device.ok_or_else(|| {
                ExternalSurfaceError::UnsupportedBackend("Not using Vulkan backend".into())
            })?;
            
            create_and_setup_resources(
                hal_device,
                &render_device,
                &mut manual_texture_views,
                &mut shared_resources,
            )
        })
    };
    
    if let Err(e) = setup_result {
        error!("Failed to setup Vulkan sharing: {}", e);
        return;
    }
    
    // Initialize IPC if configured
    if let Some(ref socket_path) = shared_resources.config.ipc_socket_path {
        #[cfg(unix)]
        {
            match IPCHandler::new_server(socket_path) {
                Ok(mut handler) => {
                    info!("IPC server initialized at {}", socket_path);
                    
                    // Send initial metadata
                    let metadata = IPCMetadata {
                        width: shared_resources.config.width,
                        height: shared_resources.config.height,
                        format: shared_resources.config.format.as_raw() as u32,
                        memory_fds: shared_resources.memory_fds.clone(),
                    };
                    
                    if let Err(e) = handler.send_initial_metadata(&metadata) {
                        error!("Failed to send initial metadata: {}", e);
                    }
                    
                    shared_resources.ipc_handler = Some(Arc::new(Mutex::new(handler)));
                }
                Err(e) => {
                    error!("Failed to initialize IPC server: {}", e);
                }
            }
        }
    }
}

unsafe fn create_and_setup_resources(
    hal_device: &wgpu_hal::vulkan::Device,
    render_device: &RenderDevice,
    manual_texture_views: &mut ManualTextureViews,
    shared_resources: &mut SharedVulkanResources,
) -> Result<()> {
    let raw_device = hal_device.raw_device();
    let raw_instance = hal_device.shared_instance().raw_instance();
    let physical_device = hal_device.raw_physical_device();
    
    // Store device for cleanup
    shared_resources.device = Some(Arc::new(raw_device.clone()));
    
    // Load extension functions
    let ext_memory_fd = ash::khr::external_memory_fd::Device::new(&raw_instance, &raw_device);
    let ext_semaphore_fd = ash::khr::external_semaphore_fd::Device::new(&raw_instance, &raw_device);
    
    // Query memory properties
    let mem_properties = unsafe { raw_instance.get_physical_device_memory_properties(physical_device) };
    
    let buffer_count = if shared_resources.config.enable_double_buffering { 2 } else { 1 };
    
    for i in 0..buffer_count {
        // Create exportable image
        let (vk_image, vk_memory, memory_fd) = unsafe { create_exportable_image_with_memory(
            &raw_device,
            &ext_memory_fd,
            &mem_properties,
            shared_resources.config.width,
            shared_resources.config.height,
            shared_resources.config.format,
        ) }?;
        
        // Create HAL texture descriptor
        let label = format!("shared_texture_{}", i);
        let hal_desc = wgpu_hal::TextureDescriptor {
            label: Some(&label),
            size: wgpu::Extent3d {
                width: shared_resources.config.width,
                height: shared_resources.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: convert_vk_format_to_wgpu(shared_resources.config.format),
            usage: wgpu_hal::TextureUses::COLOR_TARGET | wgpu_hal::TextureUses::RESOURCE,
            memory_flags: wgpu_hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        
        // Wrap the VkImage into a HAL texture
        // texture_from_raw might be an associated function in this version
        let hal_texture = unsafe {
            wgpu_hal::vulkan::Device::texture_from_raw(
                vk_image,
                &hal_desc,
                Some(Box::new(|| {})), // No-op drop callback - we manage lifetime
            )
        };
        
        // Convert HAL texture to wgpu texture
        let wgpu_label = format!("shared_wgpu_texture_{}", i);
        let wgpu_desc = wgpu::TextureDescriptor {
            label: Some(&wgpu_label),
            size: wgpu::Extent3d {
                width: shared_resources.config.width,
                height: shared_resources.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: convert_vk_format_to_wgpu(shared_resources.config.format),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        
        let wgpu_texture = render_device.wgpu_device()
            .create_texture_from_hal::<VulkanApi>(hal_texture, &wgpu_desc);
        
        // Create texture view
        let texture_view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("shared_view_{}", i)),
            ..Default::default()
        });
        
        // Create ManualTextureView
        let manual_view = ManualTextureView {
            texture_view: texture_view.into(),
            size: bevy::math::UVec2::new(
                shared_resources.config.width,
                shared_resources.config.height,
            ),
            format: convert_vk_format_to_wgpu(shared_resources.config.format),
        };
        
        // Create a unique handle for this texture view
        let handle = ManualTextureViewHandle(i as u32);
        
        // Insert the manual view with its handle
        manual_texture_views.insert(handle, manual_view);
        
        // Store everything
        shared_resources.texture_handles.push(handle);
        shared_resources.vulkan_images.push(vk_image);
        shared_resources.vulkan_memory.push(vk_memory);
        shared_resources.memory_fds.push(memory_fd);
    }
    
    // Create exportable semaphores
    for _ in 0..buffer_count {
        let (render_finished, consumer_ready) = unsafe { create_exportable_semaphores(
            &raw_device,
            &ext_semaphore_fd,
        ) }?;
        
        shared_resources.render_finished_semaphores.push(render_finished);
        shared_resources.consumer_ready_semaphores.push(consumer_ready);
    }
    
    info!("Successfully created {} shared textures and semaphores", buffer_count);
    
    Ok(())
}

unsafe fn create_exportable_image_with_memory(
    device: &ash::Device,
    ext_memory_fd: &ash::khr::external_memory_fd::Device,
    mem_properties: &vk::PhysicalDeviceMemoryProperties,
    width: u32,
    height: u32,
    format: vk::Format,
) -> Result<(vk::Image, vk::DeviceMemory, RawFd)> {
    // External memory image create info
    let mut external_memory_info = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);
    
    // Image create info
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut external_memory_info);
    
    let vk_image = unsafe { device.create_image(&image_info, None) }
        .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to create image: {:?}", e)))?;
    
    // Get memory requirements
    let mem_reqs = unsafe { device.get_image_memory_requirements(vk_image) };
    
    // Find suitable memory type
    let memory_type_index = find_memory_type(
        mem_properties,
        mem_reqs.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    
    // Export memory allocate info
    let mut export_info = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);
    
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_reqs.size)
        .memory_type_index(memory_type_index)
        .push_next(&mut export_info);
    
    let vk_memory = unsafe { device.allocate_memory(&alloc_info, None) }
        .map_err(|e| ExternalSurfaceError::MemoryExportFailed(format!("Failed to allocate memory: {:?}", e)))?;
    
    // Bind memory to image
    unsafe { device.bind_image_memory(vk_image, vk_memory, 0) }
        .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to bind memory: {:?}", e)))?;
    
    // Export memory fd
    let fd_info = vk::MemoryGetFdInfoKHR::default()
        .memory(vk_memory)
        .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);
    
    let fd = unsafe { ext_memory_fd.get_memory_fd(&fd_info) }
        .map_err(|e| ExternalSurfaceError::MemoryExportFailed(format!("Failed to export fd: {:?}", e)))?;
    
    Ok((vk_image, vk_memory, fd))
}

unsafe fn create_exportable_semaphores(
    device: &ash::Device,
    ext_semaphore_fd: &ash::khr::external_semaphore_fd::Device,
) -> Result<(vk::Semaphore, vk::Semaphore)> {
    let mut export_info = vk::ExportSemaphoreCreateInfo::default()
        .handle_types(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD);
    
    let create_info = vk::SemaphoreCreateInfo::default()
        .push_next(&mut export_info);
    
    let render_finished = unsafe { device.create_semaphore(&create_info, None) }
        .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to create semaphore: {:?}", e)))?;
    
    let consumer_ready = unsafe { device.create_semaphore(&create_info, None) }
        .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to create semaphore: {:?}", e)))?;
    
    Ok((render_finished, consumer_ready))
}

fn find_memory_type(
    mem_properties: &vk::PhysicalDeviceMemoryProperties,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> Result<u32> {
    for i in 0..mem_properties.memory_type_count {
        if (type_filter & (1 << i)) != 0 &&
           mem_properties.memory_types[i as usize].property_flags.contains(properties) {
            return Ok(i);
        }
    }
    
    Err(ExternalSurfaceError::MemoryExportFailed("No suitable memory type found".into()))
}

fn convert_vk_format_to_wgpu(format: vk::Format) -> wgpu::TextureFormat {
    match format {
        vk::Format::B8G8R8A8_SRGB => wgpu::TextureFormat::Bgra8UnormSrgb,
        vk::Format::B8G8R8A8_UNORM => wgpu::TextureFormat::Bgra8Unorm,
        vk::Format::R8G8B8A8_SRGB => wgpu::TextureFormat::Rgba8UnormSrgb,
        vk::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
        _ => wgpu::TextureFormat::Bgra8UnormSrgb,
    }
}

fn wait_for_consumer(_shared_resources: Res<SharedVulkanResources>) {
    // TODO: Implement actual semaphore waiting
    // This would involve submitting a wait operation to the GPU queue
}

fn signal_render_finished(
    mut shared_resources: ResMut<SharedVulkanResources>,
) {
    // Re-export semaphore FDs for next frame (they're consumed on import)
    if let (Some(device), Some(handler)) = (&shared_resources.device, &shared_resources.ipc_handler) {
        if let Ok(mut handler) = handler.lock() {
            // Export fresh semaphore FDs for this frame
            let current_idx = shared_resources.current_buffer_index;
            
            // TODO: Export fresh FDs for semaphores and send via IPC
            
            let frame_info = IPCFrameInfo {
                buffer_index: current_idx,
                render_finished_semaphore_fd: None, // Would be freshly exported
                consumer_ready_semaphore_fd: None,  // Would be freshly exported
            };
            
            if let Err(e) = handler.send_frame_ready(&frame_info) {
                warn!("Failed to send frame info: {}", e);
            }
        }
    }
    
    shared_resources.swap_buffers();
}

// IPC Handler implementation
#[cfg(unix)]
pub struct IPCHandler {
    socket_fd: RawFd,
    client_fd: Option<RawFd>,
}

#[cfg(unix)]
impl IPCHandler {
    fn new_server(socket_path: &str) -> Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        use std::path::Path;
        
        // Remove existing socket file
        let _ = std::fs::remove_file(socket_path);
        
        // Create socket
        let socket_fd = socket::socket(
            socket::AddressFamily::Unix,
            socket::SockType::Stream,
            socket::SockFlag::empty(),
            None,
        ).map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to create socket: {}", e)))?;
        
        // Bind to path
        let addr = UnixAddr::new(socket_path)
            .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Invalid socket path: {}", e)))?;
        
        socket::bind(socket_fd.as_raw_fd(), &addr)
            .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to bind socket: {}", e)))?;
        
        // Listen for connections
        socket::listen(&socket_fd, socket::Backlog::new(1).unwrap())
            .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to listen: {}", e)))?;
        
        Ok(Self {
            socket_fd: socket_fd.into_raw_fd(),
            client_fd: None,
        })
    }
    
    fn send_initial_metadata(&mut self, metadata: &IPCMetadata) -> Result<()> {
        // Accept client connection if not already connected
        if self.client_fd.is_none() {
            match socket::accept(self.socket_fd) {
                Ok(client_fd) => {
                    self.client_fd = Some(client_fd);
                    info!("Client connected to IPC socket");
                }
                Err(_) => {
                    // No client connected yet
                    return Ok(());
                }
            }
        }
        
        if let Some(client_fd) = self.client_fd {
            // Serialize metadata
            let data = bincode::serialize(metadata)
                .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to serialize: {}", e)))?;
            
            // Send file descriptors as ancillary data
            let fds: Vec<RawFd> = metadata.memory_fds.clone();
            let cmsg = socket::ControlMessage::ScmRights(&fds);
            
            socket::sendmsg::<()>(
                client_fd,
                &[std::io::IoSlice::new(&data)],
                &[cmsg],
                MsgFlags::empty(),
                None,
            ).map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to send metadata: {}", e)))?;
        }
        
        Ok(())
    }
    
    fn send_frame_ready(&mut self, frame_info: &IPCFrameInfo) -> Result<()> {
        if let Some(client_fd) = self.client_fd {
            // Serialize frame info
            let data = bincode::serialize(frame_info)
                .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to serialize: {}", e)))?;
            
            // Collect FDs to send (semaphores)
            let mut fds = Vec::new();
            if let Some(fd) = frame_info.render_finished_semaphore_fd {
                fds.push(fd);
            }
            if let Some(fd) = frame_info.consumer_ready_semaphore_fd {
                fds.push(fd);
            }
            
            if !fds.is_empty() {
                let cmsg = socket::ControlMessage::ScmRights(&fds);
                socket::sendmsg::<()>(
                    client_fd,
                    &[std::io::IoSlice::new(&data)],
                    &[cmsg],
                    MsgFlags::empty(),
                    None,
                ).map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to send frame info: {}", e)))?;
            } else {
                // Send without FDs
                socket::send(client_fd, &data, MsgFlags::empty())
                    .map_err(|e| ExternalSurfaceError::SurfaceCreationFailed(format!("Failed to send frame info: {}", e)))?;
            }
        }
        
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for IPCHandler {
    fn drop(&mut self) {
        unsafe {
            if let Some(client_fd) = self.client_fd {
                libc::close(client_fd);
            }
            libc::close(self.socket_fd);
        }
    }
}

// Windows stub
#[cfg(not(unix))]
pub struct IPCHandler;

#[cfg(not(unix))]
impl IPCHandler {
    fn new_server(_socket_path: &str) -> Result<Self> {
        Err(ExternalSurfaceError::UnsupportedBackend("IPC not implemented for Windows yet".into()))
    }
    
    fn send_initial_metadata(&mut self, _metadata: &IPCMetadata) -> Result<()> {
        Ok(())
    }
    
    fn send_frame_ready(&mut self, _frame_info: &IPCFrameInfo) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IPCMetadata {
    width: u32,
    height: u32,
    format: u32,
    #[cfg(unix)]
    memory_fds: Vec<RawFd>,
    #[cfg(not(unix))]
    memory_handles: Vec<isize>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IPCFrameInfo {
    buffer_index: usize,
    #[cfg(unix)]
    render_finished_semaphore_fd: Option<RawFd>,
    #[cfg(unix)]
    consumer_ready_semaphore_fd: Option<RawFd>,
    #[cfg(not(unix))]
    render_finished_semaphore_handle: Option<isize>,
    #[cfg(not(unix))]
    consumer_ready_semaphore_handle: Option<isize>,
}