use std::error::Error;
use std::fmt;

pub struct GpuContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
}

#[derive(Debug)]
pub enum GpuError {
    AdapterRequest(wgpu::RequestAdapterError),
    DeviceRequest(wgpu::RequestDeviceError),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterRequest(error) => {
                write!(f, "failed to find a suitable GPU adapter: {error}")
            }
            Self::DeviceRequest(error) => {
                write!(f, "failed to create GPU device: {error}")
            }
        }
    }
}

impl Error for GpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AdapterRequest(error) => Some(error),
            Self::DeviceRequest(error) => Some(error),
        }
    }
}

impl GpuContext {
    /// Creates a GPU context using the Vulkan backend.
    pub async fn new() -> Result<Self, GpuError> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();

        descriptor.backends = wgpu::Backends::VULKAN;

        let instance = wgpu::Instance::new(descriptor);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .map_err(GpuError::AdapterRequest)?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("NOXL GPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await
            .map_err(GpuError::DeviceRequest)?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            adapter_info,
        })
    }

    /// Creates a GPU context from synchronous code.
    pub fn new_blocking() -> Result<Self, GpuError> {
        pollster::block_on(Self::new())
    }

    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }
}
