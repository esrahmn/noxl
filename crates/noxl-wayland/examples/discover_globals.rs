
use noxl_wayland::discover_globals;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let globals = discover_globals()?;

    println!("\n=== NOXL Wayland Registry ===");

    for global in &globals {
        println!(
            "{} | version {} | name {}",
            global.interface(),
            global.version(),
            global.name()
        );
    }

    println!("\nTotal globals: {}", globals.len());

    Ok(())
}
