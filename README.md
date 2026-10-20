# NetVis

Network planning tool for Windows, written in Rust (egui/eframe). It's built for Corescale.

**Status:** Phases 0 and 1 are done: the app shell, data model, topology canvas, save/load and undo/redo.

## Build and run

### Windows (target platform)

1. Install Rust from <https://rustup.rs> and pick the default **MSVC** toolchain. The installer will offer the Visual Studio Build Tools; accept them.
2. Build and run:

```powershell
cargo run --release -p netvis-app -- samples\corescale-hq.netvis
```

The executable is written to `target\release\netvis.exe`. It's a single portable file.

### WSL / Linux (development)

```bash
cargo run -p netvis-app -- samples/corescale-hq.netvis
```

Under WSLg, if winit reports `NoCompositor`, point it at the WSLg socket:

```bash
XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir cargo run -p netvis-app
```

On Linux the native file dialogs are replaced by an in-app path prompt.

**Cross-compiling a Windows `.exe` from WSL** (optional):

```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
cargo build --release -p netvis-app --target x86_64-pc-windows-gnu
```

### Tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

`cargo run -p netvis-core --example sample` regenerates `samples/corescale-hq.netvis`.

## Using the canvas

| Action | How |
|---|---|
| Add a device or area | Drag it from the **Toolbox**, or click it to drop it at the centre of the view |
| Connect | Drag from a port to another port. Dropping on a device body picks its first free port, preferring the same media |
| Select | Click, Shift+click to add or remove, or drag on empty canvas to box-select |
| Move | Drag a device or an area header. Moving an area also moves everything inside it |
| Resize an area | Drag its bottom-right grip |
| Pan / zoom | Middle-drag or Space+drag to pan. Mouse wheel or pinch zooms at the cursor |
| Nudge | Arrow keys move by one grid step; Shift+arrow moves by 1 px |
| Undo / redo | Ctrl+Z, Ctrl+Y (or Ctrl+Shift+Z) |
| Duplicate / delete | Ctrl+D / Del |
| Fit / 100% | Ctrl+0 / Ctrl+1 |

When nothing is selected, the Inspector edits project details. With one item selected it edits that device, link or area, including each port's name, media, speed and PoE role. With several selected it offers align and distribute.

Autosave runs every 90 s while there are unsaved changes. If the app wasn't closed cleanly, it offers to recover the autosave on the next start.

## Workspace layout

```
crates/
  netvis-core/   data model, undo/redo (Edit + History), .netvis file I/O — no UI, unit-tested
  netvis-app/    eframe GUI: docking shell, canvas, panels, dialogs
samples/         example project
```

New crates are added in the phase that needs them (`netvis-ip`, `netvis-validate`, `netvis-render`, `netvis-report`, …).

## File format

A `.netvis` file is pretty-printed JSON inside a small envelope: `{"format":"netvis","version":1,"project":{…}}`. Missing fields fall back to their defaults. Files written by a newer format version are rejected with a clear message rather than misread.

## Roadmap

| Phase | Scope |
|---|---|
| 0–1 ✅ | Shell, docking layout, light/dark themes, device palette, canvas, save/load, undo/redo |
| 2 | VLANs, subnets, IPAM, per-interface IP configuration, auto-assign |
| 3 | Cable and transceiver catalogue, cable validation, PoE budget, racks |
| 4 | Conflict engine and Issues panel with auto-fixes |
| 5 | IT tools panel (calculators, OUI lookup, port reference, BOM) |
| 6 | Patch schedule and port map |
| 7 | PDF report (Corescale cover, TOC, vector diagrams, tables) |
| 8 | Auto-layout, polish, installer |

Live network tools and vendor-specific config generation are out of scope for now.
