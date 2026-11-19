use std::error::Error;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    protocol::{
        wl_callback,
        wl_registry::{self, WlRegistry},
    },
};

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

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
    layer_shell: Option<ZwlrLayerShellV1>,
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

                if interface == "zwlr_layer_shell_v1" {
                    let layer_shell =
                        registry.bind::<ZwlrLayerShellV1, _, _>(name, version.min(5), qh, ());

                    state.layer_shell = Some(layer_shell);

                    println!("NOXL: Layer Shell bound successfully!");
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

delegate_noop!(WaylandState: ignore wl_callback::WlCallback);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);

/// Maintains a connection to the Wayland compositor.
pub struct WaylandConnection {
    connection: Connection,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
}

impl WaylandConnection {
    /// Connects to the compositor and discovers available globals.
    pub fn connect() -> Result<Self, Box<dyn Error>> {
        let connection = Connection::connect_to_env()?;

        let mut event_queue = connection.new_event_queue();
        let queue_handle = event_queue.handle();

        let display = connection.display();
        display.get_registry(&queue_handle, ());

        let mut state = WaylandState::default();

        event_queue.roundtrip(&mut state)?;

        println!("\nNOXL discovered {} Wayland globals.", state.globals.len());

        Ok(Self {
            connection,
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

    /// Returns the Wayland connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Processes pending Wayland events.
    pub fn dispatch_pending(&mut self) -> Result<usize, Box<dyn Error>> {
        Ok(self.event_queue.dispatch_pending(&mut self.state)?)
    }
}
