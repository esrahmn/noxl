use noxl_wayland::WaylandConnection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = WaylandConnection::connect()?;

    println!("\n=== NOXL Wayland Registry ===");

    for global in connection.globals() {
        println!(
            "{} | version {} | name {}",
            global.interface(),
            global.version(),
            global.name()
        );
    }

    println!("\n=== Protocol Availability ===");

    println!("Compositor: {}", connection.has_global("wl_compositor"));

    println!("Layer Shell: {}", connection.has_layer_shell());

    println!("Output: {}", connection.has_global("wl_output"));

    Ok(())
}
