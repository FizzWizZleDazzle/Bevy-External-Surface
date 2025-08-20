use bevy::prelude::*;
use bevy::log::{info, warn};
use bevy::app::ScheduleRunnerPlugin;
use bevy_external_surface::vulkan_sharing::{VulkanSharingPlugin, VulkanSharingConfig, SharedVulkanResources};
use ash::vk;
use std::time::Duration;

fn main() {
    info!("Starting Bevy Vulkan Sharing Producer");
    info!("This example creates shared Vulkan textures that can be consumed by external applications");
    info!("IPC socket will be available at: /tmp/bevy_vulkan_sharing.sock");

    App::new()
        // Configure headless mode - no window creation
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::render::RenderPlugin::default(),
            bevy::render::texture::ImagePlugin::default(),
            bevy::core_pipeline::CorePipelinePlugin::default(),
            bevy::pbr::PbrPlugin::default(),
            bevy::log::LogPlugin::default(),
            ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)),
        ))
        // Add our Vulkan sharing plugin
        .add_plugins(VulkanSharingPlugin {
            config: VulkanSharingConfig {
                width: 1920,
                height: 1080,
                format: vk::Format::B8G8R8A8_SRGB,
                ipc_socket_path: Some("/tmp/bevy_vulkan_sharing.sock".to_string()),
                enable_double_buffering: true,
            },
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (animate_scene, log_sharing_status, handle_input))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create a 3D camera that will render to our shared surface
    // The VulkanSharingPlugin will automatically redirect camera output to the shared textures
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Create a ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(15.0, 15.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.3, 0.2),
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..default()
        })),
        Transform::from_xyz(0.0, -1.0, 0.0),
    ));

    // Create animated cubes in a grid pattern
    for x in -2i32..=2 {
        for z in -2i32..=2 {
            let cube_color = Color::srgb(
                0.2 + (x + 2) as f32 / 8.0,  // Red component varies with X
                0.3 + (z + 2) as f32 / 8.0,  // Green component varies with Z
                0.8 - ((x.abs() + z.abs()) as f32 / 8.0),  // Blue component varies with distance
            );
            
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.8, 0.8, 0.8))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: cube_color,
                    metallic: 0.3,
                    perceptual_roughness: 0.5,
                    reflectance: 0.8,
                    ..default()
                })),
                Transform::from_xyz(x as f32 * 2.0, 0.5, z as f32 * 2.0),
                AnimatedCube {
                    base_y: 0.5,
                    frequency: 1.0 + x as f32 * 0.3 + z as f32 * 0.2,  // Unique frequency per cube
                    amplitude: 0.6,
                    rotation_speed: 0.5 + (x.abs() + z.abs()) as f32 * 0.1,
                },
            ));
        }
    }

    // Add a rotating central light source
    commands.spawn((
        PointLight {
            intensity: 2000.0,
            range: 20.0,
            color: Color::srgb(1.0, 0.9, 0.7),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 8.0, 0.0),
        RotatingLight {
            radius: 5.0,
            height: 8.0,
            speed: 0.5,
        },
    ));

    // Add directional light for overall illumination
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            std::f32::consts::PI * 0.15,
            -std::f32::consts::PI * 0.25,
        )),
    ));

    // Add ambient light for subtle fill lighting
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.8, 0.9, 1.0),  // Slightly blue ambient
        brightness: 0.15,
        affects_lightmapped_meshes: false,
    });

    info!("Producer scene setup complete");
    info!("Rendering animated 3D scene to shared Vulkan surface");
    info!("Double-buffering enabled for smooth frame delivery");
}

#[derive(Component)]
struct AnimatedCube {
    base_y: f32,
    frequency: f32,
    amplitude: f32,
    rotation_speed: f32,
}

#[derive(Component)]
struct RotatingLight {
    radius: f32,
    height: f32,
    speed: f32,
}

fn animate_scene(
    time: Res<Time>,
    mut cubes: Query<(&mut Transform, &AnimatedCube), Without<RotatingLight>>,
    mut lights: Query<(&mut Transform, &RotatingLight), Without<AnimatedCube>>,
) {
    let elapsed = time.elapsed_secs();
    
    // Animate cubes - floating and rotating
    for (mut transform, cube) in cubes.iter_mut() {
        // Animate Y position with sinusoidal motion
        transform.translation.y = cube.base_y + 
            (elapsed * cube.frequency).sin() * cube.amplitude;
        
        // Multi-axis rotation for more dynamic movement
        transform.rotation = Quat::from_rotation_y(elapsed * cube.rotation_speed) * 
                            Quat::from_rotation_x(elapsed * cube.rotation_speed * 0.7) *
                            Quat::from_rotation_z(elapsed * cube.rotation_speed * 0.3);
    }
    
    // Animate the rotating point light
    for (mut transform, light) in lights.iter_mut() {
        let angle = elapsed * light.speed;
        transform.translation.x = angle.cos() * light.radius;
        transform.translation.z = angle.sin() * light.radius;
        transform.translation.y = light.height;
    }
}

fn log_sharing_status(
    time: Res<Time>,
    shared_resources: Res<SharedVulkanResources>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    
    // Log status every 2 seconds
    if current_time - *last_log_time >= 2.0 {
        *last_log_time = current_time;
        
        info!(
            "Vulkan Sharing Status - Frame Time: {:.2}s, Current Buffer: {}, Textures: {}",
            current_time,
            shared_resources.current_buffer_index,
            shared_resources.texture_handles.len()
        );
        
        if shared_resources.ipc_handler.is_some() {
            info!("IPC handler active - consumers can connect to receive frames");
        } else {
            warn!("IPC handler not initialized - check socket permissions");
        }
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut shared_resources: ResMut<SharedVulkanResources>,
) {
    if keys.just_pressed(KeyCode::Space) {
        info!("Manual buffer swap triggered");
        // Note: swap_buffers is called automatically in the render loop
        shared_resources.current_buffer_index = (shared_resources.current_buffer_index + 1) % shared_resources.texture_handles.len();
    }
    
    if keys.just_pressed(KeyCode::Escape) {
        info!("Producer shutting down...");
        std::process::exit(0);
    }
    
    if keys.just_pressed(KeyCode::KeyI) {
        info!("=== Vulkan Sharing Info ===");
        info!("Resolution: {}x{}", shared_resources.config.width, shared_resources.config.height);
        info!("Format: {:?}", shared_resources.config.format);
        info!("Double Buffering: {}", shared_resources.config.enable_double_buffering);
        info!("Active Textures: {}", shared_resources.texture_handles.len());
        info!("Current Buffer Index: {}", shared_resources.current_buffer_index);
        if let Some(socket_path) = &shared_resources.config.ipc_socket_path {
            info!("IPC Socket: {}", socket_path);
        }
        info!("Memory FDs: {}", shared_resources.memory_fds.len());
        info!("=== Controls ===");
        info!("SPACE: Manual buffer swap");
        info!("I: Show info");
        info!("ESC: Exit");
    }
}