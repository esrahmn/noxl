use noxl_renderer::GpuContext;

fn main() {
    println!("Initializing NOXL GPU...");

    let gpu = match GpuContext::new_blocking() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("GPU initialization failed: {error}");
            std::process::exit(1);
        }
    };

    let info = gpu.adapter_info();

    println!("\nNOXL GPU initialized!");
    println!("Name       : {}", info.name);
    println!("Vendor     : {}", info.vendor);
    println!("Device     : {}", info.device);
    println!("Backend    : {:?}", info.backend);
    println!("Device Type: {:?}", info.device_type);
}
