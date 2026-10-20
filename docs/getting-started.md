# Getting started

NetVis is a planning tool for wired and wireless networks. You draw the network, and it keeps track of the details that make a design work: VLANs and IP addresses, cables and optics, PoE power, racks and patching. It checks the design as you go and turns it into a PDF report.

This guide is built into the app (**Help → User guide**, or **F1**) and is also in the `docs` folder of the project.

## The window

| Area | What it's for |
|---|---|
| **Toolbox** (left) | Devices and areas to drag onto the canvas. Type in the search box to find one. |
| **Outline** (left, second tab) | Every area and device in the project as a list. Click one to select it. |
| **Topology** (centre) | The canvas where you draw the network. |
| **IT Tools** (centre, second tab) | Calculators and reference tools. See [IT tools](it-tools.md). |
| **Inspector** (right) | Settings for whatever is selected. With nothing selected, it edits the project details. |
| **Bottom panel** | **Issues**, **IP Plan**, **VLANs & Wi-Fi**, **Racks**, **Patch schedule** and **Port map**. |

You can drag any tab to rearrange the layout. **View → Reset panel layout** puts everything back.

## Your first design in five minutes

1. **Start a project.** Use **File → New from template…** to start from a working design (see [Templates and samples](templates.md)), or **File → New** for a blank canvas.
2. **Name it.** Click an empty part of the canvas so nothing is selected. In the Inspector, fill in **Name**, **Client** and **Revision**. These appear on the report cover.
3. **Add devices.** Drag a **Firewall / UTM**, an **L2 Switch** and a few **PCs** from the Toolbox onto the canvas.
4. **Connect them.** Drag from a port (the small squares on a device card) to a port on another device. Dropping on the device itself picks its first free port.
5. **Plan addressing.** In **VLANs & Wi-Fi**, add a VLAN. In **IP Plan**, add a subnet and bind it to the VLAN. Select the switch, open **Interfaces** in the Inspector and set the PC ports to *Access* in that VLAN. Then click **Auto-assign** in the IP Plan.
6. **Check it.** The **Issues** tab lists anything wrong, with a fix button where the remedy is clear.
7. **Share it.** **File → Export PDF report…** (Ctrl+P) creates the design report.

> Everything you do can be undone with **Ctrl+Z**, including whole operations like Auto-assign, Fix all and Auto-layout.

## Where to go next

- [Drawing the network](canvas.md): devices, areas, cables and auto-layout.
- [VLANs, subnets and Wi-Fi](ip-planning.md): the IP plan.
- [Tutorial: shopping mall](tutorial-shopping-mall.md): a complete multi-floor design, step by step.
