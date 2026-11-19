use noxl_renderer::{GpuContext, OffscreenRenderer, OffscreenTarget};

fn main() {
    println!("Initializing NOXL GPU...");

    let gpu = match GpuContext::new_blocking() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("GPU initialization failed: {error}");
            std::process::exit(1);
        }
    };

    println!("GPU: {}", gpu.adapter_info().name);

    let target = match OffscreenTarget::new(&gpu, 800, 600) {
        Ok(target) => target,
        Err(error) => {
            eprintln!("Target creation failed: {error}");
            std::process::exit(1);
        }
    };

    let renderer = OffscreenRenderer::new(&gpu);

    renderer.render(&gpu, &target);

    println!("Rendering completed!");

    let pixels = match target.read_pixels(&gpu) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("GPU readback failed: {error}");
            std::process::exit(1);
        }
    };

    let center = pixels.pixel(400, 300).expect("Pixel should exist");
    let corner = pixels.pixel(0, 0).expect("Pixel should exist");

    println!("\nReadback successful!");
    println!("Resolution : {}x{}", pixels.width(), pixels.height());
    println!("Center RGBA: {:?}", center);
    println!("Corner RGBA: {:?}", corner);

    if center[2] > center[0] && center[2] > center[1] {
        println!("Triangle pixel check: PASSED");
    } else {
        println!("Triangle pixel check: FAILED");
        std::process::exit(1);
    }

    if corner[0] == 0 && corner[1] == 0 && corner[2] == 0 {
        println!("Background pixel check: PASSED");
    } else {
        println!("Background pixel check: FAILED");
        std::process::exit(1);
    }
}
