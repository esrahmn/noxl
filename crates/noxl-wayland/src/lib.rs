use std::error::Error;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    protocol::{
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
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
    layer_shell: Option<ZwlrLayerShellV1>,
    surface: Option<WlSurface>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,
    configured_size: Option<(u32, u32)>,
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                println!("Global: {interface} (version {version}, name {name})");

                match interface.as_str() {
                    "wl_compositor" => {
                        state.compositor = Some(registry.bind(name, version.min(6), qh, ()));

                        println!("NOXL: Compositor bound!");
                    }

                    "zwlr_layer_shell_v1" => {
                        state.layer_shell = Some(registry.bind(name, version.min(5), qh, ()));

                        println!("NOXL: Layer Shell bound!");
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
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                proxy.ack_configure(serial);

                state.configured_size = Some((width, height));

                println!("NOXL: Layer Surface configured: {width}x{height}");
            }

            zwlr_layer_surface_v1::Event::Closed => {
                println!("NOXL: Layer Surface closed by compositor");
            }

            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore WlCompositor);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);
delegate_noop!(WaylandState: ignore WlSurface);

/// Maintains a connection to the Wayland compositor.
pub struct WaylandConnection {
    _connection: Connection,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
}

impl WaylandConnection {
    /// Connects to the compositor and discovers available globals.
    pub fn connect() -> Result<Self, Box<dyn Error>> {
        let connection = Connection::connect_to_env()?;

        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();

        let display = connection.display();
        display.get_registry(&qh, ());

        let mut state = WaylandState::default();

        event_queue.roundtrip(&mut state)?;

        println!("\nNOXL discovered {} Wayland globals.", state.globals.len());

        Ok(Self {
            _connection: connection,
            event_queue,
            state,
        })
    }

    /// Returns all discovered Wayland globals.
    pub fn globals(&self) -> &[WaylandGlobal] {
        &self.state.globals
    }

    /// Checks whether a Wayland global is available.
    pub fn has_global(&self, interface: &str) -> bool {
        self.state
            .globals
            .iter()
            .any(|global| global.interface() == interface)
    }

    /// Checks whether Layer Shell is available.
    pub fn has_layer_shell(&self) -> bool {
        self.state.layer_shell.is_some()
    }

    /// Creates and configures a top layer surface.
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

        layer_surface.set_size(800, 48);

        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );

        layer_surface.set_exclusive_zone(48);

        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

        // Initial commit without a buffer requests a configure event.
        surface.commit();

        self.state.surface = Some(surface);
        self.state.layer_surface = Some(layer_surface);

        // Wait for the compositor to configure the surface.
        self.event_queue.roundtrip(&mut self.state)?;

        self.state
            .configured_size
            .ok_or_else(|| "Compositor did not configure the layer surface".into())
    }
}
