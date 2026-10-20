# Patch panels and outlets

Real cabling rarely goes straight from a switch to a device. It goes from the switch through a patch cord to a **patch panel**, then along a permanent run in the ceiling to a **wall outlet** or straight to the device. NetVis models this.

## How pass-through works

Patch panels and wall outlets are *pass-through* devices. Each of their ports takes two cables: the patch cord on one side and the permanent run on the other.

1. Cable the switch port to a panel port, for example a 1 m Cat6 patch cord.
2. Cable the same panel port to the outlet or device, for example a 35 m Cat6A permanent run.
3. For an outlet, cable it to the device with another patch cord.

NetVis traces the whole run, for example `SW-L1-01:ge2 → PP-L1-01:1 → WO-L1-01:A → PC-L1-01`. Every rule then uses the two ends of the run:
- a PC behind an outlet takes its VLAN from the switch port at the other end;
- PoE is delivered and budgeted through the panel;
- the channel is checked as a whole: total length within 100 m (or the optic's reach), every cable rated for the speed, no mixing of multimode and single-mode fibre.

The same works for fibre. Use a panel with **LC** adapters, such as *12-port duplex LC fibre patch panel*, to carry an uplink: switch → floor fibre panel → riser → server-room fibre panel → core.

> A spare permanent run with nothing patched at either end is normal and isn't flagged. A device whose run stops at an unpatched panel port is flagged, because it can't be working.

## Patch schedule

The **Patch schedule** tab has one row per run, showing:
- the run's ID and both ends;
- the panel ports in between;
- every cable, with its type and the total length;
- the VLAN or trunk on the network side.

Expand a run to edit each cable's label and length.

- **Auto-label** numbers every cable from its run, such as C-001 for a direct cable or C-004.1 and C-004.2 for the cables of a patched run. Set the prefix and choose whether to replace existing labels.
- **Copy CSV** and **Save CSV…** export one line per cable, ready for the installer.

Duplicate labels are flagged in the design checks.

## Port map

The **Port map** tab draws the selected device's ports as a faceplate:
- **Colour** shows the port's state: connected, patched but going nowhere, connected on one side only, or shut down.
- **Markings:** a bar shows the access VLAN, stripes mark trunks, and ⚡ marks PoE.

Below the faceplate is a table of every port with its mode, the far end traced through any panels, and the cables and labels. Click a port to jump to the device at the other end.
