# Bevy External Surface

A powerful Bevy plugin for rendering to external surfaces, windows, and shared textures. This plugin enables headless rendering, cross-process texture sharing via Vulkan external memory, and rendering to arbitrary external windows.

## WARNING:

This crate is still experimental and has been barely tested.

## Features

- **Headless Rendering**: Run Bevy without a window for server-side rendering
- **External Window Rendering**: Render to windows created outside of Bevy
- **Vulkan External Memory**: Zero-copy texture sharing between processes
- **Flexible Surface Targets**: Support for various rendering targets
- **Cross-platform**: Works on Linux, Windows, and macOS (with platform-specific features)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy_external_surface = "0.1.0"
```

Or with specific features:

```toml
[dependencies]
bevy_external_surface = { version = "0.1.0", features = ["winit_integration"] }
```

## Quick Start

### Headless Rendering

```rust
use bevy::prelude::*;
use bevy_external_surface::{
    headless::{HeadlessRenderPlugin, HeadlessRenderSettings},
};

fn main() {
    App::new()
        .add_plugins(HeadlessRenderPlugin {
            settings: HeadlessRenderSettings {
                width: 1920,
                height: 1080,
                target_fps: 60.0,
                ..default()
            },
        })
        .add_systems(Startup, setup_scene)
        .run();
}
```

### External Window Rendering

```rust
use bevy_external_surface::{
    external_surface::{ExternalSurfacePlugin, SurfaceTarget},
};

// Create your window with winit or any other windowing library
let window = create_external_window();

App::new()
    .add_plugins(ExternalSurfacePlugin {
        target: SurfaceTarget::Window(window),
        size: (1280, 720),
        ..default()
    })
    .run();
```

### Vulkan External Memory (Zero-Copy Texture Sharing)

```rust
use bevy_external_surface::vulkan_interop::{
    VulkanExternalTexture, ExternalMemoryHandle,
};

// In the sender process
let texture = VulkanExternalTexture::create_exportable(
    &render_device,
    size,
    format,
)?;

// Export the memory handle
let handle = texture.memory_handle.unwrap();

// In the receiver process
let imported_texture = VulkanExternalTexture::import_from_handle(
    &render_device,
    handle,
    size,
    format,
)?;
```

## Examples

Run the examples with:

```bash
# Headless rendering example
cargo run --example headless_render

# External window example (requires winit_integration feature)
cargo run --example external_window --features winit_integration

# Shared texture example (demonstrates Vulkan external memory)
cargo run --example shared_texture
```

## Architecture

The plugin is structured into three main modules:

1. **`external_surface`**: Core functionality for rendering to external surfaces
2. **`headless`**: Headless rendering configuration and setup
3. **`vulkan_interop`**: Vulkan external memory and synchronization primitives

### How It Works

1. **Headless Mode**: Replaces Bevy's default windowing plugins with a custom plugin group that excludes window creation while maintaining the full rendering pipeline.

2. **External Surfaces**: Provides trait-based abstraction for different surface types (windows, textures, raw GPU resources).

3. **Vulkan Interop**: Uses `wgpu`'s HAL escape hatch to access native Vulkan handles, enabling external memory export/import for zero-copy texture sharing.

## Platform Support

| Feature | Linux | Windows | macOS |
|---------|-------|---------|-------|
| Headless Rendering | ✅ | ✅ | ✅ |
| External Windows | ✅ | ✅ | ✅ |
| Vulkan External Memory (FD) | ✅ | ❌ | ❌ |
| Vulkan External Memory (Win32) | ❌ | ✅ | ❌ |
| D3D11 Texture Sharing | ❌ | ✅ | ❌ |

## Requirements

- Bevy 0.15+
- Rust 1.75+
- For Vulkan features: Vulkan 1.2+ with appropriate extensions

## Performance Considerations

- **Zero-Copy Sharing**: Eliminates CPU-GPU round trips for inter-process rendering
- **Headless Mode**: Reduces overhead by skipping window system integration
- **External Surfaces**: Allows integration with existing rendering pipelines

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

Dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Acknowledgments

This plugin builds upon the excellent work of the Bevy community and leverages the powerful abstraction layers provided by `wgpu` and `wgpu-hal`.
