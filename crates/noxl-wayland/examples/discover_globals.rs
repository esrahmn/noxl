use noxl_wayland::WaylandConnection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to Wayland...");

    let mut connection = WaylandConnection::connect()?;

    println!("\n=== NOXL Protocol Availability ===");

    println!("Compositor: {}", connection.has_global("wl_compositor"));

    println!("Layer Shell: {}", connection.has_layer_shell());

    println!("\n=== Creating Layer Surface ===");

    let (width, height) = connection.create_layer_surface()?;

    println!("\nNOXL Layer Surface created!");
    println!("Configured size: {width}x{height}");

    Ok(())
}
