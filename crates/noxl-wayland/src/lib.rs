use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    protocol::{
        wl_callback,
        wl_registry::{self, WlRegistry},
    },
};

#[derive(Default)]
struct WaylandState {
    globals: Vec<WaylandGlobal>,
}

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

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            println!("Global: {interface} (version {version}, name {name})");

            state.globals.push(WaylandGlobal {
                name,
                interface,
                version,
            });
        }
    }
}

delegate_noop!(WaylandState: ignore wl_callback::WlCallback);

/// Connects to the current Wayland compositor and discovers its globals.
pub fn discover_globals() -> Result<Vec<WaylandGlobal>, Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;

    let mut event_queue = connection.new_event_queue();
    let queue_handle = event_queue.handle();

    let display = connection.display();
    display.get_registry(&queue_handle, ());

    let mut state = WaylandState::default();

    event_queue.roundtrip(&mut state)?;

    println!("\nDiscovered {} Wayland globals.", state.globals.len());

    Ok(state.globals)
}
