use std::{error::Error, fs::File, os::fd::AsFd};

use memmap2::MmapMut;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};

use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};

#[derive(Debug)]
pub struct WaylandGlobal {
    name: u32,
    interface: String,
    version: u32,
}

impl WaylandGlobal {
    pub fn name(&self) -> u32 {
        self.name
    }

    pub fn interface(&self) -> &str {
        &self.interface
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Default)]
struct WaylandState {
    globals: Vec<WaylandGlobal>,
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,
    surface: Option<WlSurface>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,
    buffer: Option<ShmBuffer>,
    configured_size: Option<(u32, u32)>,
    closed: bool,
}

// Menjaga seluruh resource SHM tetap hidup selama buffer digunakan.
struct ShmBuffer {
    _file: File,
    _mapping: MmapMut,
    _pool: WlShmPool,
    buffer: WlBuffer,
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                println!("Global: {interface} v{version}");

                match interface.as_str() {
                    "wl_compositor" => {
                        state.compositor = Some(registry.bind(name, version.min(6), qh, ()));
                    }
                    "wl_shm" => {
                        state.shm = Some(registry.bind(name, version.min(2), qh, ()));
                    }
                    "zwlr_layer_shell_v1" => {
                        state.layer_shell = Some(registry.bind(name, version.min(5), qh, ()));
                    }
                    _ => {}
                }

                state.globals.push(WaylandGlobal {
                    name,
                    interface,
                    version,
                });
            }
            wl_registry::Event::GlobalRemove { name } => {
                println!("Global removed: {name}");
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                proxy.ack_configure(serial);
                state.configured_size = Some((width, height));
                println!("NOXL: configured {width}x{height}");
            }
            zwlr_layer_surface_v1::Event::Closed => {
                println!("NOXL: compositor closed the surface");
                state.closed = true;
            }
            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore WlCompositor);
delegate_noop!(WaylandState: ignore WlSurface);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);
delegate_noop!(WaylandState: ignore WlShm);
delegate_noop!(WaylandState: ignore WlShmPool);
delegate_noop!(WaylandState: ignore WlBuffer);

pub struct WaylandConnection {
    _connection: Connection,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
}

impl WaylandConnection {
    pub fn connect() -> Result<Self, Box<dyn Error>> {
        let connection = Connection::connect_to_env()?;
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();

        connection.display().get_registry(&qh, ());

        let mut state = WaylandState::default();
        event_queue.roundtrip(&mut state)?;

        println!("\nNOXL discovered {} Wayland globals.", state.globals.len());

        Ok(Self {
            _connection: connection,
            event_queue,
            state,
        })
    }

    pub fn globals(&self) -> &[WaylandGlobal] {
        &self.state.globals
    }

    pub fn has_global(&self, interface: &str) -> bool {
        self.state
            .globals
            .iter()
            .any(|g| g.interface() == interface)
    }

    pub fn has_layer_shell(&self) -> bool {
        self.state.layer_shell.is_some()
    }

    fn create_shm_buffer(&self, width: u32, height: u32) -> Result<ShmBuffer, Box<dyn Error>> {
        let shm = self.state.shm.as_ref().ok_or("wl_shm is unavailable")?;

        let width_i32 = i32::try_from(width)?;
        let height_i32 = i32::try_from(height)?;
        let stride = width_i32.checked_mul(4).ok_or("Stride overflow")?;
        let size = (stride as usize)
            .checked_mul(height as usize)
            .ok_or("Buffer size overflow")?;
        let size_i32 = i32::try_from(size)?;

        // Gunakan shared-memory filesystem, dengan fallback ke temp directory.
        let file = tempfile::tempfile_in("/dev/shm").or_else(|_| tempfile::tempfile())?;

        file.set_len(size as u64)?;

        let mut mapping = unsafe { MmapMut::map_mut(&file)? };

        // ARGB8888 pada mesin little-endian tersimpan sebagai BGRA.
        // Warna panel: gelap, dengan alpha penuh.
        for pixel in mapping.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0x25, 0x1b, 0x18, 0xff]);
        }

        let qh = self.event_queue.handle();

        let pool = shm.create_pool(file.as_fd(), size_i32, &qh, ());

        let buffer = pool.create_buffer(
            0,
            width_i32,
            height_i32,
            stride,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );

        Ok(ShmBuffer {
            _file: file,
            _mapping: mapping,
            _pool: pool,
            buffer,
        })
    }

    pub fn create_layer_surface(&mut self) -> Result<(u32, u32), Box<dyn Error>> {
        let qh = self.event_queue.handle();

        let compositor = self
            .state
            .compositor
            .as_ref()
            .ok_or("Wayland compositor is unavailable")?;

        let layer_shell = self
            .state
            .layer_shell
            .as_ref()
            .ok_or("Wayland Layer Shell is unavailable")?;

        let surface = compositor.create_surface(&qh, ());

        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Top,
            "noxl".to_string(),
            &qh,
            (),
        );

        // Lebar 0 berarti mengikuti lebar area yang tersedia.
        layer_surface.set_size(0, 48);

        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );

        layer_surface.set_exclusive_zone(48);

        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

        // Commit awal tanpa buffer untuk meminta configure.
        surface.commit();

        self.state.surface = Some(surface);
        self.state.layer_surface = Some(layer_surface);

        // Tunggu ukuran dari compositor.
        self.event_queue.roundtrip(&mut self.state)?;

        let (width, height) = self
            .state
            .configured_size
            .ok_or("Compositor did not configure the layer surface")?;

        if width == 0 || height == 0 {
            return Err("Compositor returned an invalid surface size".into());
        }

        let buffer = self.create_shm_buffer(width, height)?;

        let surface = self
            .state
            .surface
            .as_ref()
            .ok_or("Surface is unavailable")?;

        surface.attach(Some(&buffer.buffer), 0, 0);
        surface.damage(0, 0, width as i32, height as i32);
        surface.commit();

        self.state.buffer = Some(buffer);

        println!("NOXL: SHM buffer attached and committed!");

        Ok((width, height))
    }

    /// Copies RGBA pixels into the SHM buffer and presents the frame.
    pub fn present_rgba(&mut self, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
        let (width, height) = self
            .state
            .configured_size
            .ok_or("Surface has not been configured")?;

        let expected_size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|size| size.checked_mul(4))
            .ok_or("Pixel buffer size overflow")?;

        if rgba.len() != expected_size {
            return Err(format!(
                "Invalid pixel buffer size: expected {expected_size}, got {}",
                rgba.len()
            )
            .into());
        }

        let buffer = self
            .state
            .buffer
            .as_mut()
            .ok_or("SHM buffer is unavailable")?;

        // Wayland ARGB8888 on little-endian machines uses BGRA byte order.
        for (dst, src) in buffer
            ._mapping
            .chunks_exact_mut(4)
            .zip(rgba.chunks_exact(4))
        {
            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }

        let surface = self
            .state
            .surface
            .as_ref()
            .ok_or("Surface is unavailable")?;

        surface.damage(0, 0, width as i32, height as i32);
        surface.commit();

        Ok(())
    }

    /// Keep the surface alive until the compositor closes it.
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        println!("NOXL: event loop running. Press Ctrl+C to exit.");

        while !self.state.closed {
            self.event_queue.blocking_dispatch(&mut self.state)?;
        }

        Ok(())
    }
}
