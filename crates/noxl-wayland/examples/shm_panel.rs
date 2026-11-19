use noxl_renderer::{GpuContext, OffscreenRenderer, OffscreenTarget};

use noxl_wayland::WaylandConnection;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== NOXL GPU Panel ===");

    // 1. Connect to Wayland.
    let mut connection = WaylandConnection::connect()?;

    println!("\n=== Protocol Availability ===");
    println!("Compositor: {}", connection.has_global("wl_compositor"));
    println!("SHM: {}", connection.has_global("wl_shm"));
    println!("Layer Shell: {}", connection.has_layer_shell());

    // 2. Create the Wayland layer surface.
    let (width, height) = connection.create_layer_surface()?;

    println!("\nPanel size: {width}x{height}");

    // 3. Initialize GPU.
    println!("\n=== Initializing GPU ===");

    let gpu = GpuContext::new_blocking()?;

    println!("GPU: {}", gpu.adapter_info().name);
    println!("Backend: {:?}", gpu.adapter_info().backend);

    // 4. Create an offscreen render target.
    let target = OffscreenTarget::new(&gpu, width, height)?;

    // 5. Create the renderer.
    let renderer = OffscreenRenderer::new(&gpu);

    // 6. Render a GPU test frame.
    println!("\n=== Rendering ===");

    renderer.render(&gpu, &target);

    // 7. Read rendered pixels back from GPU.
    let pixels = target.read_pixels(&gpu)?;

    println!("GPU frame rendered successfully!");

    // 8. Present pixels through Wayland SHM.
    connection.present_rgba(pixels.as_bytes())?;

    println!("Frame presented through Wayland!");

    // 9. Keep the panel alive.
    connection.run()?;

    Ok(())
}
