# NetVis

Network planning tool for Windows, written in Rust (egui/eframe). It's built for Corescale.

**Status:** Phases 0–3 are done: the app shell, topology canvas, save/load, undo/redo, VLANs, subnets, IPAM, interface configuration, and the physical layer (cables, optics, PoE, racks) with live validation.

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

## IP planning

- **VLANs & Wi-Fi tab:** the VLAN database (tag, name, purpose, colour) and wireless networks (SSID, VLAN, security, bands). Click a row to edit it in the Inspector.
- **IP Plan tab:** the subnet tree. Child subnets are indented under their parent block, and each row has a utilisation bar showing child subnets, assigned addresses, reserved ranges, the DHCP pool and free space.
  - **VLSM planner:** splits an address block into right-sized subnets (largest first). It can create the matching VLANs, a gateway on the first host and a DHCP scope over the upper half, all as one undoable step.
  - **Auto-assign:** gives every unaddressed host port and VLAN interface the next free address in the subnet bound to its VLAN. If devices are selected, it only covers those. A host's VLAN comes from the switch port it's cabled to (access VLAN, or the native VLAN on a trunk). An SVI takes the subnet gateway, and each host gets that gateway as its default. The DHCP pool, reserved ranges and addresses already in use are always skipped. Interfaces it couldn't address are listed with the reason.
- **Subnet inspector:** a built-in calculator (mask, wildcard, broadcast, host range, scope), the gateway, the DHCP scope (range, lease, DNS, serving device), reserved ranges, and every address in the subnet. Click an address to jump to its device.
- **Device inspector:** four sections.
  - **General:** hostname, vendor, model, DNS override and an address summary.
  - **Ports:** the port table. Click a port's mode to open its interface settings.
  - **Interfaces:** for ports, the switchport mode (host, access, trunk with native and allowed VLANs, or routed), MTU, duplex, MAC, PoE and radio settings. You can also add SVIs, loopbacks, 802.1Q sub-interfaces and tunnels. Every interface has IP addressing with **Next free**, a default gateway, and inline warnings for duplicate addresses, addresses outside any planned subnet, the network or broadcast address, prefix mismatches, and static addresses inside a DHCP pool.
  - **Routing:** static routes, NAT rules (source PAT, static NAT, port forward) and notes on dynamic routing.
- **Project inspector** (nothing selected): project-wide DNS, NTP, syslog and domain settings.
- **Canvas:** port outlines take their access VLAN's colour; trunk ports have a bold outline. Device cards show their primary IP (toggle under View → Show IP addresses).

Deleting a VLAN clears every reference to it on ports, SVIs, subnets and SSIDs, in one step you can undo.

## Physical layer

- **Device models:** pick one under General → Hardware to set the port layout, rack height, power draw and PoE budget in one step. The library is vendor-neutral and lives in [crates/netvis-catalog/data/models.json](crates/netvis-catalog/data/models.json); edit that file to add your own models. Ports with the same name keep their cabling and configuration, and the inspector warns before any links would be removed.
- **Transceivers:** each SFP/QSFP cage has a module picker (SX/LX/ZX, 10G SR/LR/ER, 25G, 40G SR4/LR4, 100G SR4/CWDM4/LR4, plus 1G and 10G copper SFP modules). Older modules work in newer cages (SFP in SFP+, QSFP+ in QSFP28).
- **Cable checks:**
  - **Media:** fibre into an RJ45 port, copper into an empty SFP cage, a DAC on an RJ45 port, a wireless link on a wired port.
  - **Optics:** a module that doesn't fit its cage, mismatched optics (SR at one end, LR at the other), an optic on the wrong fibre type.
  - **Speed:** mismatches between the two ends. Optics and DACs are errors because they don't auto-negotiate. On copper it's a warning between two network devices, a note when a switch port limits an AP or end device, and nothing when the end device is simply the slower side.
  - **Length:** checked against the cable's rated length at the link speed (e.g. Cat6 at 10G is limited to 55 m) and against the optics' reach.
  - **Duplex:** mismatches, and one end forced to full while the other auto-negotiates.
- **PoE:** each switch's budget is the sum of what its PSE ports deliver to the devices they power. A device uses its measured draw if one is set, otherwise its class maximum, scaled up for worst-case cable loss. Over the budget is an error; over 90% is a warning. A device whose class is higher than its switch port provides gets a warning, as does a powered device on a port with no PoE. Patch panels count as passive and are skipped.
- **Racks:** every Rack area on the canvas is a rack, and the Racks tab shows its elevation. Drag devices to move them (the outline turns green when the slot is free), right-click to remove one, and place unmounted devices with one click or **Auto-place** (network gear from the top; servers, storage and UPS from the bottom). It reports overlaps, devices that don't fit the rack, and devices drawn in a rack but not placed in it. Each rack shows its power load against its UPS capacity (warning above 80%, error above 100%).
- **Issues tab:** every finding, filterable by severity. Click one to select and centre the item involved. The inspector also lists the findings for the selected item, and the canvas marks devices and links that have errors or warnings with red or amber badges.

Autosave runs every 90 s while there are unsaved changes. If the app wasn't closed cleanly, it offers to recover the autosave on the next start.

## Workspace layout

```
crates/
  netvis-ip/       IPv4/IPv6 subnet math, VLSM, summarisation, next-free search — pure, unit-tested
  netvis-catalog/  physical-layer reference data: media, cable reach, transceivers, PoE, device models (JSON)
  netvis-core/     data model, VLANs/subnets/IPAM, hardware/racks/PoE, undo/redo, .netvis file I/O — no UI
  netvis-validate/ design-rule engine (cabling, power, racks) producing Issues
  netvis-app/      eframe GUI: docking shell, canvas, panels, dialogs
samples/         example project
```

New crates are added in the phase that needs them (`netvis-render`, `netvis-report`, …).

## File format

A `.netvis` file is pretty-printed JSON inside a small envelope: `{"format":"netvis","version":1,"project":{…}}`. Missing fields fall back to their defaults, so files saved by older versions still open. Files written by a newer format version are rejected with a clear message rather than misread.

## Roadmap

| Phase | Scope |
|---|---|
| 0–1 ✅ | Shell, docking layout, light/dark themes, device palette, canvas, save/load, undo/redo |
| 2 ✅ | VLANs, subnets, IPAM, per-interface IP configuration, auto-assign |
| 3 ✅ | Cable and transceiver catalogue, cable validation, PoE budget, racks |
| 4 | IP/VLAN/Wi-Fi conflict rules and one-click auto-fixes (the Issues panel and engine already exist) |
| 5 | IT tools panel (calculators, OUI lookup, port reference, BOM) |
| 6 | Patch schedule and port map |
| 7 | PDF report (Corescale cover, TOC, vector diagrams, tables) |
| 8 | Auto-layout, polish, installer |

Live network tools and vendor-specific config generation are out of scope for now.
