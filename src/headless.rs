use bevy::{
    app::{PluginGroupBuilder, ScheduleRunnerPlugin},
    prelude::*,
    render::{
        camera::RenderTarget,
        render_asset::RenderAssetUsages,
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages,
        },
    },
};
use std::time::Duration;


#[derive(Resource, Clone)]
pub struct HeadlessRenderSettings {
    pub width: u32,
    pub height: u32,
    pub target_fps: f32,
    pub format: TextureFormat,
}

impl Default for HeadlessRenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            target_fps: 60.0,
            format: TextureFormat::Bgra8UnormSrgb,
        }
    }
}

pub struct HeadlessRenderPlugin {
    pub settings: HeadlessRenderSettings,
}

impl Default for HeadlessRenderPlugin {
    fn default() -> Self {
        Self {
            settings: HeadlessRenderSettings::default(),
        }
    }
}

impl Plugin for HeadlessRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone());
        
        app.add_plugins(HeadlessPlugins);
        
        app.add_plugins(
            ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / self.settings.target_fps as f64,
            ))
        );
        
        app.add_systems(Startup, setup_headless_camera);
    }
}

pub struct HeadlessPlugins;

impl PluginGroup for HeadlessPlugins {
    fn build(self) -> PluginGroupBuilder {
        let mut group = PluginGroupBuilder::start::<Self>();
        
        // Just add the essential plugins for headless rendering
        group = group
            .add(bevy::asset::AssetPlugin::default())
            .add(bevy::render::RenderPlugin::default())
            .add(bevy::render::texture::ImagePlugin::default())
            .add(bevy::core_pipeline::CorePipelinePlugin::default())
            .add(bevy::pbr::PbrPlugin::default());
        
        group
    }
}

fn setup_headless_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    settings: Res<HeadlessRenderSettings>,
) {
    let size = Extent3d {
        width: settings.width,
        height: settings.height,
        depth_or_array_layers: 1,
    };
    
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        settings.format,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC
        | TextureUsages::COPY_DST;
    
    let image_handle = images.add(image);
    
    commands.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(image_handle.clone().into()),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    commands.insert_resource(HeadlessRenderTarget {
        image: image_handle,
    });
}

#[derive(Resource)]
pub struct HeadlessRenderTarget {
    pub image: Handle<Image>,
}