# Templates and samples

**File → New from template…** (Ctrl+Shift+N) opens the template gallery. A template is a complete, working design: VLANs, subnets, addresses, cabling, racks and labels are all filled in, and every template passes all [design checks](design-checks.md). Pick the closest one and change it to suit, which is usually quicker than starting blank.

A template opens as a new, unsaved project. Use **File → Save As…** to keep it.

| Template | Level | What's in it |
|---|---|---|
| **Home network** | Basic | Internet router, small PoE switch, one Wi-Fi AP, a PC, TV, NAS and printer on one LAN |
| **Small office** | Basic | 10–20 people. Firewall routing staff, voice, guest and management VLANs (router-on-a-stick), one PoE switch and patch panel in a wall cabinet, two APs, desks with IP phones |
| **Retail store** | Basic | Tills on their own VLAN (keeps card payments separate), cameras and a recorder, back office, customer Wi-Fi that only reaches the internet |
| **IP CCTV system** | Basic | Stand-alone surveillance network: 20 cameras on two PoE switches with fibre uplinks, recording server, storage, monitoring PC |
| **Small company** | Intermediate | 50–80 people on two floors. L3 core routing six VLANs, server rack, fibre uplinks per floor, desks on wall outlets, Wi-Fi |
| **Head office (Corescale HQ)** | Intermediate | Firewall, core, DMZ, server rack, two floors with a fibre riser and patched desks |
| **Shopping mall (tutorial)** | Advanced | The finished design from the [shopping-mall tutorial](tutorial-shopping-mall.md): nine floors with CCTV, door access, Wi-Fi and background music |
| **Campus (Corescale Campus)** | Advanced | Data centre with dual ISPs and a core pair, VM hosts and storage, two buildings and five floors |

## Ideas for using templates

- **Learn by example.** Open a template, select things and look at the Inspector. Every switch port has its VLAN set, every AP has a channel, and every cable has a type, length and label.
- **Copy a part.** Select part of a template, change it, and use **Edit → Duplicate with rename…** to repeat it. See [Drawing the network](canvas.md).
- **Try a change and see what breaks.** Swap a switch for a smaller model, or delete a trunk, and watch the Issues tab.

The files are in the `templates` and `samples` folders of the project, and the installer copies the samples to `Program Files\NetVis\samples`.
