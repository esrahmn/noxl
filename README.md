# NOXL

**A native UI framework for Linux, built with Rust.**

NOXL is an experimental native UI framework focused on building GPU-accelerated desktop interfaces for modern Linux environments.

Designed around Rust, `wgpu`, and Wayland, NOXL aims to provide the foundations for custom desktop interfaces, reusable UI components, reactive state, and smooth animations — without relying on existing UI frameworks.

> **Status:** Early development · Pre-alpha · APIs are subject to change.

---

## Vision

The goal of NOXL is to make building native Linux desktop interfaces more flexible and accessible through a modular, GPU-driven architecture.

Rather than being a desktop compositor or a traditional widget toolkit, NOXL is designed as a framework for creating custom Linux desktop applications and shell components.

The long-term vision includes:

- GPU-accelerated rendering.
- A flexible layout engine.
- Reactive UI state and component updates.
- Animation and transition systems.
- Native Wayland integration.
- Platform services and desktop integrations.
- A declarative UI language.

## Architecture

NOXL is organized as a modular Rust workspace. Each crate is responsible for a specific part of the framework.

| Crate | Responsibility |
|---|---|
| `noxl-core` | Core types, geometry, colors, components, and UI tree |
| `noxl-renderer` | GPU context, rendering pipeline, draw lists, and primitives |
| `noxl-wayland` | Wayland integration |
| `noxl-layout` | Layout calculation and sizing |
| `noxl-reactive` | Reactive state system |
| `noxl-animation` | Animation and transition system |
| `noxl-widgets` | Reusable UI components |
| `noxl-platform` | Native desktop platform integration |
| `noxl-runtime` | UI runtime |
| `noxl-scripting` | Scripting support |
| `noxl-cli` | Command-line tooling |

## Technology

- **Language:** Rust
- **Graphics:** wgpu
- **Graphics API:** Vulkan
- **Windowing environment:** Wayland
- **Target platform:** Linux

## Development Roadmap

NOXL is being developed incrementally, with a focus on building a reliable foundation before introducing higher-level abstractions.

- [x] Rust workspace structure
- [x] Core geometry and color types
- [x] Component tree foundation
- [x] Layout engine foundation
- [x] GPU initialization
- [x] Offscreen rendering foundation
- [ ] Complete rendering pipeline
- [ ] Wayland surface integration
- [ ] Input and event handling
- [ ] Reactive state system
- [ ] UI component lifecycle
- [ ] Animation engine
- [ ] Native desktop integrations
- [ ] Declarative UI language
- [ ] Developer tooling and documentation

*The roadmap represents development goals, not a guarantee of completed functionality.*

## Project Goals

NOXL is being built with a few principles in mind:

- **Native first** — designed for Linux desktop environments.
- **GPU driven** — rendering is built around modern GPU APIs.
- **Modular** — independent crates with clearly separated responsibilities.
- **Flexible** — custom interfaces without being locked into a predefined visual system.
- **Rust powered** — safety, performance, and maintainable systems code.

## Development Environment

The initial development target is:

- CachyOS / Arch Linux
- Wayland
- Hyprland
- Rust stable toolchain
- Vulkan-capable GPU

Other Wayland environments may be supported as development progresses.

## Building from Source

Make sure you have Rust and Cargo installed.

Clone the repository:

```bash
git clone https://github.com/esrahmn/noxl.git
cd noxl
```

Check the workspace:

```bash
cargo check --workspace
```

Build the workspace:

```bash
cargo build --workspace
```

## Contributing

NOXL is currently in early development. The architecture, internal APIs, and project structure may change as the framework evolves.

Bug reports, ideas, and technical discussions are welcome.

Before contributing major changes, consider opening an issue to discuss the proposed direction.

## License

NOXL is licensed under the [MIT License](LICENSE).

---

<p align="center">
  <strong>NOXL</strong><br>
  <sub>Building native interfaces, from the ground up.</sub>
</p>
