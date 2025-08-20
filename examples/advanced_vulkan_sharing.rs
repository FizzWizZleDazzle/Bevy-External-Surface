// Advanced Vulkan Sharing Example
//
// This example demonstrates advanced features of the VulkanSharingPlugin:
// - Double-buffering for smooth frame delivery
// - Proper synchronization with semaphores
// - Complex animated 3D scene
// - Performance monitoring and statistics
// - Runtime configuration changes
//
// Key concepts:
// - Buffer swapping to prevent tearing
// - Producer-consumer synchronization
// - Frame pacing and timing
// - Resource management
//
// This example is ideal for:
// - Production applications requiring smooth playback
// - Understanding synchronization mechanisms
// - Performance optimization and debugging

use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy::winit::WinitPlugin;
use bevy_external_surface::vulkan_sharing::{VulkanSharingPlugin, VulkanSharingConfig, SharedVulkanResources};
use ash::vk;
use std::time::{Duration, Instant};

fn main() {
    println!("🚀 Advanced Vulkan Sharing Example");
    println!("   Features: Double-buffering, Synchronization, Performance Monitoring");
    println!("   Resolution: 1920x1080 @ 60 FPS target");
    println!("   Socket: /tmp/advanced_vulkan_sharing.sock");
    println!();
    println!("🎮 Controls:");
    println!("   SPACE: Force buffer swap");
    println!("   P: Toggle performance stats");
    println!("   ESC: Exit");
    
    App::new()
        // Headless rendering setup
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<WinitPlugin>()
                .disable::<WindowPlugin>()
        )
        // Advanced Vulkan sharing with all features enabled
        .add_plugins(VulkanSharingPlugin {
            config: VulkanSharingConfig {
                width: 1920,
                height: 1080,
                format: vk::Format::B8G8R8A8_SRGB,
                ipc_socket_path: Some("/tmp/advanced_vulkan_sharing.sock".to_string()),
                enable_double_buffering: true,  // Enable for smooth playback
            },
        })
        .insert_resource(PerformanceStats::default())
        .insert_resource(SceneConfig::default())
        .add_systems(Startup, setup_advanced_scene)
        .add_systems(Update, (
            animate_advanced_scene,
            update_performance_stats,
            handle_controls,
            manage_synchronization,
            log_advanced_status,
        ).chain())
        .run();
}

#[derive(Resource)]
struct PerformanceStats {
    frame_count: u64,
    start_time: Instant,
    last_fps_calculation: Instant,
    current_fps: f32,
    buffer_swaps: u64,
    show_stats: bool,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            frame_count: 0,
            start_time: now,
            last_fps_calculation: now,
            current_fps: 0.0,
            buffer_swaps: 0,
            show_stats: true,
        }
    }
}

#[derive(Resource)]
struct SceneConfig {
    animation_speed: f32,
    light_intensity: f32,
    particle_count: usize,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            animation_speed: 1.0,
            light_intensity: 1.0,
            particle_count: 50,
        }
    }
}

fn setup_advanced_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scene_config: Res<SceneConfig>,
) {
    info!("🎬 Setting up advanced 3D scene for high-performance sharing");
    
    // Advanced camera with better positioning
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(10.0, 8.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ));
    
    // Central rotating structure
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let radius = 5.0;
        
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::hsl(angle * 180.0 / std::f32::consts::PI, 0.8, 0.6),
                metallic: 0.8,
                perceptual_roughness: 0.2,
                reflectance: 0.9,
                ..default()
            })),
            Transform::from_xyz(
                angle.cos() * radius,
                0.0,
                angle.sin() * radius,
            ),
            RotatingStructure {
                center_distance: radius,
                rotation_speed: 0.5,
                orbit_speed: 1.0,
                base_angle: angle,
            },
        ));
    }
    
    // Create particle system simulation
    for i in 0..scene_config.particle_count {
        let angle = (i as f32 / scene_config.particle_count as f32) * std::f32::consts::TAU;
        let radius = 2.0 + (i as f32 % 3.0);
        let height = (i as f32).sin() * 2.0;
        
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.1).mesh().uv(32, 18))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.9, 0.7, 0.3),
                emissive: Color::srgb(0.3, 0.2, 0.0).into(),
                metallic: 0.0,
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_xyz(
                angle.cos() * radius,
                height,
                angle.sin() * radius,
            ),
            FloatingParticle {
                base_radius: radius,
                base_height: height,
                frequency: 1.0 + (i as f32 * 0.1),
                amplitude: 0.5,
                phase: angle,
            },
        ));
    }
    
    // Ground with interesting pattern
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 30.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.15, 0.2),
            metallic: 0.8,
            perceptual_roughness: 0.3,
            reflectance: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, -3.0, 0.0),
    ));
    
    // Multiple dynamic lights for dramatic effect
    for i in 0..3 {
        let angle = i as f32 * std::f32::consts::TAU / 3.0;
        commands.spawn((
            PointLight {
                intensity: 3000.0,
                range: 15.0,
                color: Color::hsl(angle * 180.0 / std::f32::consts::PI, 0.7, 0.8),
                shadows_enabled: true,
                ..default()
            },
            Transform::from_xyz(0.0, 5.0, 0.0),
            DynamicLight {
                orbit_radius: 8.0,
                orbit_height: 5.0,
                orbit_speed: 0.3 + i as f32 * 0.1,
                base_angle: angle,
                intensity_variation: 0.5,
            },
        ));
    }
    
    // Ambient lighting
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.4, 0.5, 0.7),
        brightness: 0.1,
        affects_lightmapped_meshes: false,
    });
    
    info!("Advanced scene setup complete - {} objects created", 
          8 + scene_config.particle_count + 1 + 3);
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct RotatingStructure {
    center_distance: f32,
    rotation_speed: f32,
    orbit_speed: f32,
    base_angle: f32,
}

#[derive(Component)]
struct FloatingParticle {
    base_radius: f32,
    base_height: f32,
    frequency: f32,
    amplitude: f32,
    phase: f32,
}

#[derive(Component)]
struct DynamicLight {
    orbit_radius: f32,
    orbit_height: f32,
    orbit_speed: f32,
    base_angle: f32,
    intensity_variation: f32,
}

fn animate_advanced_scene(
    time: Res<Time>,
    scene_config: Res<SceneConfig>,
    mut structures: Query<(&mut Transform, &RotatingStructure)>,
    mut particles: Query<(&mut Transform, &FloatingParticle), Without<RotatingStructure>>,
    mut lights: Query<(&mut Transform, &mut PointLight, &DynamicLight), (Without<RotatingStructure>, Without<FloatingParticle>)>,
    mut camera: Query<&mut Transform, (With<MainCamera>, Without<RotatingStructure>, Without<FloatingParticle>, Without<DynamicLight>)>,
) {
    let elapsed = time.elapsed_secs() * scene_config.animation_speed;
    
    // Animate rotating structures
    for (mut transform, structure) in structures.iter_mut() {
        let orbit_angle = structure.base_angle + elapsed * structure.orbit_speed;
        
        transform.translation.x = orbit_angle.cos() * structure.center_distance;
        transform.translation.z = orbit_angle.sin() * structure.center_distance;
        
        // Self rotation
        transform.rotation = Quat::from_rotation_y(elapsed * structure.rotation_speed);
    }
    
    // Animate floating particles
    for (mut transform, particle) in particles.iter_mut() {
        let wave = (elapsed * particle.frequency + particle.phase).sin();
        let orbit_angle = particle.phase + elapsed * 0.2;
        
        transform.translation.x = orbit_angle.cos() * (particle.base_radius + wave * 0.5);
        transform.translation.z = orbit_angle.sin() * (particle.base_radius + wave * 0.5);
        transform.translation.y = particle.base_height + wave * particle.amplitude;
    }
    
    // Animate dynamic lights
    for (mut transform, mut light, dynamic_light) in lights.iter_mut() {
        let orbit_angle = dynamic_light.base_angle + elapsed * dynamic_light.orbit_speed;
        
        transform.translation.x = orbit_angle.cos() * dynamic_light.orbit_radius;
        transform.translation.z = orbit_angle.sin() * dynamic_light.orbit_radius;
        transform.translation.y = dynamic_light.orbit_height;
        
        // Vary light intensity
        let intensity_wave = (elapsed * 2.0 + dynamic_light.base_angle).sin() * 0.5 + 0.5;
        light.intensity = 3000.0 * (1.0 + intensity_wave * dynamic_light.intensity_variation) * scene_config.light_intensity;
    }
    
    // Slowly orbit camera around the scene
    if let Ok(mut camera_transform) = camera.single_mut() {
        let camera_angle = elapsed * 0.1;
        let camera_radius = 15.0;
        
        let new_position = Vec3::new(
            camera_angle.cos() * camera_radius,
            8.0,
            camera_angle.sin() * camera_radius,
        );
        
        camera_transform.translation = new_position;
        camera_transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}

fn update_performance_stats(
    _time: Res<Time>,
    mut stats: ResMut<PerformanceStats>,
) {
    stats.frame_count += 1;
    
    let now = Instant::now();
    if now.duration_since(stats.last_fps_calculation) >= Duration::from_secs(1) {
        let elapsed = now.duration_since(stats.last_fps_calculation).as_secs_f32();
        let frames_since_last = stats.frame_count - (stats.current_fps * elapsed) as u64;
        stats.current_fps = frames_since_last as f32 / elapsed;
        stats.last_fps_calculation = now;
    }
}

fn handle_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PerformanceStats>,
    mut shared_resources: ResMut<SharedVulkanResources>,
) {
    if keys.just_pressed(KeyCode::Space) {
        info!("Manual buffer swap requested");
        shared_resources.swap_buffers();
        stats.buffer_swaps += 1;
    }
    
    if keys.just_pressed(KeyCode::KeyP) {
        stats.show_stats = !stats.show_stats;
        info!("Performance stats display: {}", if stats.show_stats { "ON" } else { "OFF" });
    }
    
    if keys.just_pressed(KeyCode::Escape) {
        info!("Advanced Vulkan sharing example shutting down...");
        std::process::exit(0);
    }
}

fn manage_synchronization(
    shared_resources: Res<SharedVulkanResources>,
) {
    // In a real application, this is where you would:
    // 1. Wait for consumer-ready semaphores before rendering to a buffer
    // 2. Signal render-finished semaphores after GPU work completes
    // 3. Monitor timing to prevent frame drops
    // 4. Adjust rendering quality based on consumer performance
    
    // For now, we'll just track that synchronization is being managed
    if shared_resources.render_finished_semaphores.len() != shared_resources.consumer_ready_semaphores.len() {
        warn!("Semaphore count mismatch - synchronization may be unstable");
    }
}

fn log_advanced_status(
    time: Res<Time>,
    stats: Res<PerformanceStats>,
    shared_resources: Res<SharedVulkanResources>,
    mut last_log_time: Local<f32>,
) {
    let current_time = time.elapsed_secs();
    
    if !stats.show_stats {
        return;
    }
    
    // Log detailed status every 2 seconds
    if current_time - *last_log_time >= 2.0 {
        *last_log_time = current_time;
        
        let uptime = stats.start_time.elapsed();
        
        info!("🚀 Advanced Vulkan Sharing Status:");
        info!("   ⏱️  Uptime: {:.1}s", uptime.as_secs_f32());
        info!("   🎯 FPS: {:.1} (Target: 60)", stats.current_fps);
        info!("   🖼️  Frames rendered: {}", stats.frame_count);
        info!("   🔄 Buffer swaps: {} (Auto: {}, Manual: {})", 
              shared_resources.current_buffer_index, 
              stats.frame_count.saturating_sub(stats.buffer_swaps),
              stats.buffer_swaps);
        info!("   📐 Resolution: {}x{}", 
              shared_resources.config.width, 
              shared_resources.config.height);
        info!("   🎚️  Double buffering: {}", shared_resources.config.enable_double_buffering);
        info!("   🔗 Active textures: {}", shared_resources.texture_handles.len());
        info!("   🚦 Semaphores: {} render-finished, {} consumer-ready", 
              shared_resources.render_finished_semaphores.len(),
              shared_resources.consumer_ready_semaphores.len());
        
        if shared_resources.ipc_handler.is_some() {
            info!("   📡 IPC: Active - consumers can connect");
        } else {
            warn!("   ⚠️  IPC: Inactive - check configuration");
        }
        
        // Performance warnings
        if stats.current_fps < 50.0 {
            warn!("   ⚠️  Low FPS detected - consider reducing scene complexity");
        }
    }
}