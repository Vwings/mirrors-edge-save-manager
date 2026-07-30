# Mirror's Edge Save Switcher

An unofficial, open-source save preset and recovery manager for Mirror's Edge.

The project is currently a technical prototype used to validate Slint, FemtoVG,
Windows packaging, and binary size. It does not modify save files yet, and the
prototype UI is not the final product design.

The current product and architecture decisions are documented in
[docs/design.md](docs/design.md). The path to a releasable first version is in
[docs/roadmap.md](docs/roadmap.md), while [docs/status.md](docs/status.md)
records the current implementation handoff.

## Technology

- Rust 2024
- Slint with the Winit backend
- FemtoVG with OpenGL rendering
- Windows 10/11 x64

Only the required Slint features are enabled. Skia, WGPU, the software renderer,
and system tray support are intentionally excluded from the prototype.

## Development

```powershell
cargo run
cargo test
cargo build --release
```

The release executable is written to
`target\release\mirrors-edge-save-switcher.exe`.

The current x64 technical prototype builds to a 7.12 MiB Windows GUI executable
with the MSVC runtime linked statically. Its remaining imports are Windows system
libraries, including `OPENGL32.dll` for FemtoVG rendering.

## Internationalization

User-visible strings are marked with Slint's `@tr()` macro from the start. The
prototype ships only its English source strings; translation catalogs can be
bundled into the executable later without changing the UI structure.

## License

Licensed under the GNU General Public License v3.0 only.

This project is not affiliated with or endorsed by Electronic Arts or DICE.
