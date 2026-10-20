# Cables, optics, PoE and racks

## Cables

Every cable has a type and, ideally, a length. The checks compare them with the ports at each end:

| Check | Example |
|---|---|
| **Media** | Fibre into an RJ45 port, copper into an empty SFP cage, a DAC on an RJ45 port |
| **Speed** | Cat5e on a 10G link; ends running at different speeds |
| **Length** | Cat6 is limited to 55 m at 10G and copper channels to 100 m; fibre is checked against the optic's reach |
| **Duplex** | One end forced to full duplex while the other negotiates |

On patched runs, the whole channel from switch to device is checked, not each cable on its own. See [Patch panels and outlets](patching.md).

## Optics

Each SFP/QSFP cage has a **Module** picker in the cable's Inspector and on the Interfaces tab. Options include:
- 1G SX/LX/ZX;
- 10G SR/LR/ER;
- 25G, 40G SR4/LR4, and 100G SR4/CWDM4/LR4;
- copper SFP modules.

The checks flag:
- a module that doesn't fit its cage;
- different optics at the two ends (SR at one end, LR at the other);
- an optic on the wrong kind of fibre;
- a run longer than the optic's reach.

Older modules work in newer cages (SFP in SFP+, QSFP+ in QSFP28).

> Single-mode fibre (OS2) with 10GBASE-LR optics reaches 10 km, which is plenty for any building riser. Multimode (OM4) with SR optics is cheaper for short runs, up to about 400 m at 10G.

## PoE

Each switch's PoE budget comes from its model, for example 370 W on a 24-port PoE+ switch.

- **Load:** NetVis adds up what each powered device needs. That's its measured draw if the model has one, otherwise the maximum for its PoE class, plus an allowance for cable loss.
- **Over budget:** going over the budget is an error. Over 90% is a warning.
- **Class mismatch:** a device that needs a higher class than its port supplies is flagged. An example is a PTZ camera that needs 802.3bt on an 802.3at port.
- **Through panels:** power is traced through patch panels, so a camera patched through a panel still counts against its switch.

The **PoE budget** tool in IT Tools shows the same sums for one switch. Use **Load from switch** to fill it in.

## Racks

Draw a **Rack** area on the canvas around the devices it holds, and set its height (in U) in the Inspector. The **Racks** tab shows each rack's front elevation.

- **Auto-place:** places every unmounted device in the rack, network gear from the top and servers, storage and UPS from the bottom.
- **Moving and removing:** drag a device to move it; the outline turns green when the slot is free. Right-click a device to take it out.
- **Power:** each rack shows its power load against its UPS capacity. Above 80% is a warning and above 100% is an error.

The checks also report overlapping devices, devices that don't fit, and devices drawn inside a rack area but not placed in it.
