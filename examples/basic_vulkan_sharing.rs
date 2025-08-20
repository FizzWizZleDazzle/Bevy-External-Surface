// Basic Vulkan Sharing Example
//
// This example demonstrates the simplest possible use of VulkanSharingPlugin.
// It creates a single shared texture without double-buffering and renders
// a basic 3D scene that can be consumed by external applications.
//
// Key features:
// - Single texture sharing (no double-buffering)
// - Minimal scene setup
// - Basic error handling
// - Clear logging for debugging
//
// This is ideal for:
// - Learning the basics of Vulkan texture sharing
// - Testing integration with external applications
// - Prototyping texture sharing workflows

use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy::winit::WinitPlugin;
use bevy_external_surface::{VulkanSharingPlugin, VulkanSharingConfig, SharedVulkanResources};
use ash::vk;

fn main() {
    println!("🔰 Basic Vulkan Sharing Example");
    println!("   Single texture sharing without double-buffering");
    println!("   Socket: /tmp/basic_vulkan_sharing.sock");
    
    App::new()
        // Headless rendering - no window needed
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .disable::<WindowPlugin>()
        )
        // Simple Vulkan sharing configuration
        .add_plugins(VulkanSharingPlugin {
            config: VulkanSharingConfig {
                width: 1280,
                height: 720,
                format: vk::Format::B8G8R8A8_SRGB,
                ipc_socket_path: Some("/tmp/basic_vulkan_sharing.sock".to_string()),
                enable_double_buffering: false,  // Keep it simple - single texture
            },
        })
        .add_systems(Startup, setup_basic_scene)
        .add_systems(Update, (rotate_objects, log_status))
        .run();
}

fn setup_basic_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up basic 3D scene for sharing");
    
    // Simple camera setup
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 3.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // A simple cube to render
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.2),
            metallic: 0.0,
            perceptual_roughness: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RotatingObject { speed: 1.0 },
    ));
    
    // A simple plane as ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(8.0, 8.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(0.0, -2.0, 0.0),
    ));
    
    // Basic directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            std::f32::consts::PI * 0.25,
            -std::f32::consts::PI * 0.25,
        )),
    ));
    
    // Ambient light for visibility
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
        affects_lightmapped_meshes: false,
    });
    
    info!("Basic scene setup complete");
    info!("Rendering to shared Vulkan texture (1280x720)");
}

#[derive(Component)]
struct RotatingObject {
    speed: f32,
}

fn rotate_objects(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &RotatingObject)>,
) {
    for (mut transform, rotating) in query.iter_mut() {
        transform.rotate_y(time.delta_secs() * rotating.speed);
    }
}

fn log_status(
    time: Res<Time>,
    shared_resources: Res<SharedVulkanResources>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    
    // Log every 3 seconds
    if current_time - *last_log_time >= 3.0 {
        *last_log_time = current_time;
        
        info!("📊 Basic Sharing Status:");
        info!("   ⏱️  Runtime: {:.1}s", current_time);
        info!("   📐 Resolution: {}x{}", 
              shared_resources.config.width, 
              shared_resources.config.height);
        info!("   🖼️  Shared textures: {}", shared_resources.texture_handles.len());
        info!("   🔄 Double buffering: {}", shared_resources.config.enable_double_buffering);
        
        if shared_resources.ipc_handler.is_some() {
            info!("   🔗 IPC: Ready for consumers");
        } else {
            warn!("   ⚠️  IPC: Not initialized");
        }
        
        info!("   💡 Connect a consumer to see the shared texture!");
    }
}