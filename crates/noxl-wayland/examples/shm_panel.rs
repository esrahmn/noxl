use noxl_wayland::WaylandConnection;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== NOXL SHM Panel ===");

    let mut connection = WaylandConnection::connect()?;

    println!("\n=== Protocol Availability ===");
    println!("Compositor: {}", connection.has_global("wl_compositor"));
    println!("SHM: {}", connection.has_global("wl_shm"));
    println!("Layer Shell: {}", connection.has_layer_shell());

    println!("\n=== Creating Panel ===");

    let (width, height) = connection.create_layer_surface()?;

    println!("NOXL panel ready: {width}x{height}");

    connection.run()?;

    Ok(())
}
