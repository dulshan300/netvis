# Drawing the network

## Devices

Drag a device from the **Toolbox** onto the canvas, or click it to drop it in the middle of the view. Devices are grouped as **Network** (routers, switches, firewalls, APs…), **End devices** (PCs, servers, phones, cameras…), **Infrastructure** (patch panels, wall outlets, UPS, PDU…) and **Cloud & WAN** (Internet, ISP, MPLS, VPN…).

Each device gets a name from its type (SW-01, CAM-02…). Rename it in the Inspector under **General**. Names must be unique, and should be valid hostnames.

> A device's port layout comes from its **model**. Pick one under **General → Hardware → Model** to get the right number and type of ports, its rack height, power draw and PoE budget. See [Devices, ports and interfaces](devices.md).

## Areas

Areas group devices: **Site**, **Building**, **Floor**, **Rack**, **VLAN zone** and **Security zone**. Find them in the Toolbox under **Areas & zones**.

- **Contents:** a device belongs to an area when its card's centre is inside it. Areas can be nested, for example a Rack inside a Floor inside a Building.
- **Moving and resizing:** drag an area by its header to move it together with everything inside it. Drag the grip at the bottom-right corner to resize it.
- **Racks:** every Rack area appears in the **Racks** tab, where you place devices in rack units. See [Cables, optics, PoE and racks](physical.md).
- **Where areas matter:** the report gives each building, floor or rack its own diagram page when the overall drawing is too large, and Wi-Fi checks treat APs on the same floor as neighbours.

## Connecting devices

Drag from a port to a port on another device. Dropping on a device body picks its first free port, preferring the same type (copper to copper, SFP to SFP).

Click a cable to select it. In the Inspector you can then set:
- the **Cable** type (Cat6, Cat6A, OM4, OS2, DAC…);
- the **Length**, which is checked against the cable's limits;
- the **Label**, which appears in the patch schedule;
- the **Module** at each end, if the port takes an optic.

Port squares are filled in the cable's colour when connected. Their outline shows the access VLAN's colour, and trunk ports have a bold outline.

## Selecting

- **Click** to select one item. **Shift+click** adds or removes items from the selection.
- **Drag on empty canvas** to select everything in a box.
- **Ctrl+A** selects everything.

With several devices selected, the Inspector offers **Align** and **Distribute**, also found in the **Arrange** menu.

## Copying

- **Duplicate** (Ctrl+D) copies the selected devices, and the cables between them, with new generic names.
- **Duplicate with rename…** (Ctrl+Shift+D) is built for repeated floors and racks.
  1. Select a floor area by clicking its header, along with any extra devices.
  2. Open **Edit → Duplicate with rename…**.
  3. Enter **Find** `L1` and **Replace with** `L2`, and choose where to place the copy.
  4. The result: the area, every area and device inside it, and the cables between them are all copied. `SW-L1-SEC` becomes `SW-L2-SEC`, `ELV L1` becomes `ELV L2`, cable labels are renamed the same way, and devices keep their rack positions in the copied rack.

Cables from the copied devices to the outside, such as uplinks, aren't copied. Add those afterwards.

## Auto-layout

**Arrange → Auto-layout diagram** (Ctrl+Shift+L, or the tree button on the toolbar) redraws the whole diagram in layers, following the cables from the top of the network down: Internet, firewall, core, access switches, patch panels, outlets and devices.

- **Areas are kept.** Each area is rebuilt around its devices, areas never overlap, and every device stays in the area it was in.
- **Ordering:** devices are ordered to keep cables short. Outlets follow the order of the panel ports they're patched to.
- **Undo:** the whole layout is one step; press Ctrl+Z to go back.

**Arrange → Arrange selection** lays out only the selected devices, in place, and leaves areas alone. It's handy for tidying a single floor.

## Viewing

| Action | How |
|---|---|
| Pan | Middle-drag, or hold Space and drag |
| Zoom | Mouse wheel or pinch (zooms at the cursor); Ctrl+= and Ctrl+- |
| Fit / 100% | Ctrl+0 / Ctrl+1 |
| Show IP addresses | **View → Show IP addresses on devices** |
| Grid, snap, minimap | **View** menu or the toolbar |
| Light or dark theme | **View → Theme** |
