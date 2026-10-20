# Tutorial: shopping mall

In this tutorial you design the ELV network of a nine-storey shopping mall, from an empty canvas to a finished PDF report. Along the way you use most of NetVis: VLAN and IP planning, areas and racks, device models, patch panels, a fibre backbone, PoE budgets, Wi-Fi channels, design checks and the report.

The finished design is available as a template: **File → New from template… → Shopping mall (tutorial)**. Keep it open in a second window to compare as you work, or start from it and follow along to see how each part was built.

> Plan on about two hours for a first pass. Steps 5–7 build one floor by hand; after that, **Duplicate with rename** copies it to the other floors in a few clicks.

## The brief

The mall has nine floors: **BF, GF, L1, L2, L3, L4, L4A, L5 and L6**.

- **Every floor** has IP CCTV (8–18 cameras), Wi-Fi (3–15 access points) and two door access controllers.
- **BF to L3** also have network-based background music (BGM) with IP ceiling speakers.
- **ELV rooms:** every floor has an ELV room with one rack holding two switches.
- **Server room:** on GF.
- **Backbone:** every floor connects to the server room over a fibre backbone.
- **L4 and L4A** are stacked.

| Floor | Use | Cameras | APs | Door controllers | BGM speakers |
|---|---|---|---|---|---|
| L6 | Food court and roof | 10 | 6 | 2 | — |
| L5 | Cinema | 10 | 8 | 2 | — |
| L4A | Mezzanine | 8 | 3 | 2 | — |
| L4 | Retail | 12 | 10 | 2 | — |
| L3 | Retail | 14 | 12 | 2 | 6 |
| L2 | Retail | 14 | 12 | 2 | 6 |
| L1 | Retail | 16 | 14 | 2 | 8 |
| GF | Atrium and server room | 18 | 15 | 2 | 8 |
| BF | Car park and supermarket | 18 | 6 | 2 | 6 |

### How the brief is read

These decisions turn the brief into a design. Change them if your project differs.

- **Two switches per ELV room, one per group of systems.**
  - The **security switch** (`SW-<floor>-SEC`) serves the cameras and door controllers. It talks to the video-management and access-control servers.
  - The **services switch** (`SW-<floor>-SVC`) serves the Wi-Fi APs and BGM speakers. It talks to the Wi-Fi controller and the BGM server.

  Splitting them keeps the security systems apart from public Wi-Fi.
- **L4 and L4A are one switch stack.** L4A's switches join the L4 switches over fibre, and only L4 uplinks to the server room.
- **Every field device is patched.** Each one runs through the floor's 48-port patch panel, so every cable appears in the patch schedule.

## Step 1: Plan before you draw

A few minutes with a table saves hours on the canvas.

**VLANs and subnets:** one VLAN per system, routed by the core switch in the server room.

| VLAN | Name | Subnet | Addresses | Used by |
|---|---|---|---|---|
| 10 | Management | 10.50.10.0/24 | Static | Switches, APs, UPS, Wi-Fi controller |
| 20 | CCTV | 10.50.20.0/23 | Static | ~120 cameras, video servers, recording storage |
| 30 | Access control | 10.50.30.0/24 | Static | Door controllers, access-control server |
| 40 | BGM | 10.50.40.0/24 | Static | Speakers, BGM server |
| 50 | Wi-Fi Ops | 10.50.64.0/22 | DHCP | Mall operations staff |
| 60 | Wi-Fi Tenant | 10.50.80.0/21 | DHCP | Shop tenants |
| 70 | Wi-Fi Guest | 10.60.0.0/20 | DHCP | Shoppers (internet only) |
| — | Transit FW-core | 10.50.255.0/30 | Static | Firewall to core |
| — | ISP handoff | 203.0.113.16/29 | Static | Firewall WAN |

**Names:** put the floor in every name. *Duplicate with rename* depends on this.

| Device | Name |
|---|---|
| Security switch | `SW-L1-SEC` |
| Services switch | `SW-L1-SVC` |
| Copper patch panel | `PP-L1-01` |
| Fibre patch panel | `FPP-L1-01` |
| UPS | `UPS-L1-01` |
| Camera | `CAM-L1-01` … |
| Access point | `AP-L1-01` … |
| Door controller | `ACS-L1-01` |
| Speaker | `SPK-L1-01` … |
| ELV rack | `ELV L1` |

**Power over Ethernet:** this decides the switch models. Allow 30 W per AP (PoE+ class 4) and 15.4 W per camera, door controller or speaker (802.3af).

| Floor switch | Worst case | Model |
|---|---|---|
| GF security | 18 cameras + 2 doors = 20 × 15.4 W = 308 W | 24-port PoE+ (370 W) is enough |
| GF services | 15 APs × 30 W + 8 speakers × 15.4 W = 573 W | Needs the 48-port PoE+ (740 W) |
| L4 services | 10 APs × 30 W = 300 W | 24-port is enough |

> Use **IT Tools → PoE budget** to try these sums. Later, **Load from switch** fills it from the design.

## Step 2: Project details

Click an empty part of the canvas. In the Inspector:
1. Set **Name** to *Shopping Mall*, **Client** to the owner's name, and **Revision** to *A*.
2. Write a one-line **Description**. It goes on the report cover.
3. Under **Network services**, set the domain (*mall.local*), DNS servers (*1.1.1.1, 8.8.8.8*) and NTP (*pool.ntp.org*).

## Step 3: VLANs, subnets and Wi-Fi

1. **VLANs:** open the **VLANs & Wi-Fi** tab and click **Add VLAN** seven times. Set the tags and names from the table in step 1, and type a short purpose for each.
2. **Subnets:** open the **IP Plan** tab and click **Add subnet** for each row of the table. Select each subnet and set its **VLAN**. Keep the gateway on the first address. Tick **DHCP scope** on the three Wi-Fi subnets.

   > The **VLSM planner…** can do this in one go: give it 10.50.0.0/16 and the host counts, and it creates the subnets, VLANs, gateways and DHCP scopes together. It sizes subnets largest first, so the result differs from the table above. Both are fine.

3. **SSIDs:** back in **VLANs & Wi-Fi**, click **Add SSID** three times.

| SSID | VLAN | Security | Bands |
|---|---|---|---|
| Mall-Ops | 50 Wi-Fi Ops | WPA3-Enterprise | 5 GHz |
| Mall-Tenant | 60 Wi-Fi Tenant | WPA2/WPA3 transition | 5 GHz |
| Mall-Free-WiFi | 70 Wi-Fi Guest | OWE (Enhanced Open) | 5 GHz |

## Step 4: The building and its floors

1. From **Toolbox → Areas & zones**, drag a **Building** onto the canvas and name it *Shopping Mall*. Make it large; you can resize it any time with the grip at the bottom-right.
2. Drag a **Floor** into the building for each level, stacked top to bottom: L6 at the top and BF at the bottom, like a riser diagram. Name them so the floor code comes first, for example *L1 · Level 1 · Retail*. Make each floor about 2,800 wide and 350–550 high. GF is taller because it holds the server room.
3. **Racks:** inside each floor, at the left, drag a **Rack** named *ELV L1* (and so on for each floor). In the Inspector set **Rack height** to 24U.
   > For now, make just the L1 floor and its rack. You'll copy the rest in step 8.
4. **Server room:** in GF, below the ELV rack, drag a **Security zone** named *GF Server Room*. Put two 42U racks in it: *SR-01 Network* and *SR-02 Servers*.

## Step 5: The server room

Place these devices and pick each **Model** under **General → Hardware**.

| Rack | Device | Model |
|---|---|---|
| SR-01 Network | `FW-01` (Firewall / UTM) | Campus firewall 16×1G, 8×SFP+ |
| SR-01 Network | `CSW-01`, `CSW-02` (L3 Switch) | Distribution switch 24×SFP+, 4×QSFP+ |
| SR-01 Network | `SW-SR-01` (L2 Switch) | 48-port non-PoE access switch |
| SR-01 Network | `FPP-SR-01`, `FPP-SR-02` (Patch Panel) | 24-port duplex LC fibre patch panel |
| SR-01 Network | `UPS-SR-01` | Online UPS 3000 VA |
| SR-02 Servers | `VMS-01`, `VMS-02` (Server) | 2U server, video management and recording |
| SR-02 Servers | `NAS-CCTV-01`, `NAS-CCTV-02` (NAS / SAN) | 12-bay NAS, recording storage |
| SR-02 Servers | `ACS-SRV-01`, `BGM-SRV-01` (Server) | 1U server |
| SR-02 Servers | `WLC-01` (Wi-Fi Controller) | — |
| SR-02 Servers | `UPS-SR-02`, `UPS-SR-03` | Online UPS 3000 VA |

Put an **ISP** cloud (`ISP-01`) outside the building, next to GF.

Then cable and configure the server room:

1. **Internet:** connect `ISP-01` to `FW-01`'s first port. On the firewall's **Interfaces** tab, set that port to **Routed** with address *203.0.113.18/29* and gateway *203.0.113.17*.
2. **Firewall to core:**
   - **Cable:** connect `FW-01` x1 to `CSW-01` xe1 with a 1 m **DAC**.
   - **Addresses:** set both ends to **Routed**, with *10.50.255.1/30* on the firewall and *10.50.255.2/30* on the core.
3. **Routing:** on the **Routing** tab, give the firewall:
   - static routes for 10.50.0.0/16 and 10.60.0.0/20 via *10.50.255.2*;
   - a default route via *203.0.113.17*;
   - a **Source PAT** rule for internet access.

   Give `CSW-01` a default route via *10.50.255.1*.
4. **Core pair:**
   - **Cables:** connect `CSW-01` et1↔`CSW-02` et1 and et2↔et2 with DACs.
   - **Ports:** set all four ports to **Trunk** (native 10, allowed *All*) with **LAG group** 1.
5. **Gateways:** on `CSW-01`, under Interfaces, click **VLAN interface (SVI)** once for each VLAN, 10 to 70. These are the gateways. Give `CSW-02` an SVI in VLAN 10 so it can be managed.
6. **Recording:** cable `VMS-01/02` and `NAS-CCTV-01/02` to `CSW-01` xe10–xe13 with DACs, and set those core ports to **Access**, VLAN 20.
7. **Server switch:**
   - **Uplink:** cable `CSW-01` xe14 to `SW-SR-01` xe1 (trunk, native 10).
   - **Servers:** cable `ACS-SRV-01` to an access port in VLAN 30, `BGM-SRV-01` in VLAN 40, and `WLC-01` in VLAN 10.
   - **UPS management:** cable the three UPS management ports to access ports in VLAN 10.

## Step 6: One ELV room (L1)

In the *ELV L1* rack, place:

| Device | Model |
|---|---|
| `FPP-L1-01` | 12-port duplex LC fibre patch panel |
| `SW-L1-SEC` | 24-port PoE+ access switch, 4×SFP+ (370 W) |
| `SW-L1-SVC` | 48-port PoE+ access switch, 4×SFP+ (740 W). Use the 24-port on floors where the PoE sum allows. |
| `PP-L1-01` | 48-port Cat6A patch panel |
| `UPS-L1-01` | Line-interactive UPS 1500 VA |

Cable the UPS management port to the last copper port of `SW-L1-SVC`, and set that port to **Access**, VLAN 10. Give each switch a **VLAN interface (SVI)** in VLAN 10 for management.

## Step 7: The field devices on L1

Place the devices to the right of the rack, one row per system.

| Devices | Count on L1 | Model | Network |
|---|---|---|---|
| `CAM-L1-01` … | 16 | *Fixed dome camera* or *Bullet camera with IR* | Security switch |
| `AP-L1-01` … | 14 | *Wi-Fi 6 AP 2×2* | Services switch |
| `ACS-L1-01`, `ACS-L1-02` (IoT device) | 2 | *Door access controller* | Security switch |
| `SPK-L1-01` … (IoT device) | 8 | *IP ceiling speaker* | Services switch |

> Make one camera, set its model, then select it and press **Ctrl+D** to duplicate. **Arrange → Arrange selection** lines the copies up. Rename them in the Inspector.

Now patch each device through the panel, one panel port per device:

1. **Patch cord:** drag from a switch port, say `SW-L1-SEC` ge1, to panel port 1 of `PP-L1-01`. Set the cable to **Cat6**, **1 m**.
2. **Permanent run:** drag from the same panel port 1 to the camera. Set the cable to **Cat6A** and enter the real length, for example *35 m*.
3. **Switch port:** on the switch's **Interfaces** tab, set the port's mode.

| Device | Switch port mode |
|---|---|
| Cameras | **Access**, VLAN 20 |
| Door controllers | **Access**, VLAN 30 |
| Speakers | **Access**, VLAN 40 |
| APs | **Trunk**, native VLAN 10, allowed 10, 50, 60, 70. The SSIDs need their VLANs on the AP's uplink, and the AP is managed on the native VLAN. |

**Wi-Fi channels:** select each AP and open **Interfaces → Radio**. Set the width to **20 MHz** and give every AP on the floor its own 5 GHz channel: 36, 40, 44, 48, 52, 56, 60, 64, 100, 104… With 14 APs on a floor, 80 MHz channels would overlap. The design checks flag that, and their fix picks a free channel.

> To check your work, select `CAM-L1-01` and open the **Port map** tab, or look at its run in the **Patch schedule**. It should read `SW-L1-SEC:ge1 → PP-L1-01:1 → CAM-L1-01`, with the camera's VLAN shown as 20.

## Step 8: Copy the floor to every level

1. **Select the floor:** click the header of the L1 floor area.
2. **Copy it:** use **Edit → Duplicate with rename…** (Ctrl+Shift+D). It suggests *L1* in **Find**. Type *L2* in **Replace with**, choose **Above**, and click **Duplicate**.

   You get a new floor with its rack, switches, panels and devices, all renamed: `SW-L2-SEC`, `CAM-L2-01`, `ELV L2`. The cables between them are copied too, and the switches keep their rack positions.
3. **Repeat:** do the same for L3, L4, L4A, L5, L6, GF and BF. Copy each from the nearest floor and place it above or below.
4. **Adjust each floor to the brief:**
   - **Remove extras:** delete surplus cameras and APs, for example L4A keeps 8 cameras and 3 APs.
   - **Add what's missing:** add devices where the floor needs more. GF needs 18 cameras, 15 APs and 8 speakers, so it uses 43 of the 48 panel ports.
   - **Speakers:** delete the speakers on L4, L4A, L5 and L6, which have no background music.
   - **Smaller switches:** where the services PoE sum is under 370 W, select `SW-xx-SVC` and change its model to the 24-port switch. The cables on ports 1–24 are kept.
5. **Fix the floor names:** rename the copied floor areas, for example *L2 · Level 2 · Retail*.

> If a name was already taken, the copy gets a numbered name instead and the status bar tells you how many. Rename those by hand.

## Step 9: The fibre backbone

Each floor's switches reach the core through two fibre panels:

```
SW-L1-SEC:xe1 → FPP-L1-01:1 → (riser, 36 m) → FPP-SR-01:3 → CSW-01:xe4
SW-L1-SVC:xe1 → FPP-L1-01:2 → (riser, 36 m) → FPP-SR-02:3 → CSW-02:xe4
```

For each floor except L4A:
1. **Patch cords:** cable the switch's first SFP+ port to the floor fibre panel, ports 1 (security) and 2 (services), with **OS2** single-mode, 2 m.
2. **Riser:** cable the floor panel port to the next free port on `FPP-SR-01` (security) or `FPP-SR-02` (services). Use OS2 with the real riser length; the sample uses 30 m on GF, adding 6 m for each floor up or down.
3. **Core:** cable the server-room panel port to the next free SFP+ port on `CSW-01` (security) or `CSW-02` (services), with OS2, 2 m.
4. **Optics:** fit a **10GBASE-LR** module at both ends, on the switch and on the core port. Click the cable and use **Module A / Module B**, or use the fix the design checks offer.
5. **Port modes:** set both ends to **Trunk**, native VLAN 10, allowed *All*.

> Single-mode fibre with LR optics is the safe choice for a riser: it reaches 10 km, so any floor can be connected from the server room without a closer look at the reach. The fibre-loss check confirms each run is within budget.

## Step 10: Stack L4A with L4

L4A doesn't get its own backbone. Instead:

1. **Security switch:** cable `SW-L4A-SEC` xe1 → `FPP-L4A-01` port 1 → (8 m riser) → `FPP-L4-01` port 3 → `SW-L4-SEC` xe2.
2. **Services switch:** do the same for `SW-L4A-SVC`, using panel ports 2 and 4 and `SW-L4-SVC` xe2.
3. **Optics and ports:** fit 10GBASE-LR at both ends and set the ports to **Trunk**, native 10.
4. **Notes:** in each switch's **Notes**, record the stack membership, for example *Stack member 2 with SW-L4-SEC*. The notes appear in the device inventory of the report.

All of L4A's traffic now uses L4's uplinks, and the stack shows in the diagram as a short fibre link between the two ELV rooms.

## Step 11: Racks, addresses and labels

1. **Racks:** in the **Racks** tab, select each rack and click **Auto-place**. Check the power bar of each rack.

   *SR-02 Servers* draws about 2,400 W: two 600 W video servers, two NAS units and two 1U servers. One 2,700 W UPS would be at 89%, which is why the design has two UPS units in that rack.
2. **Spanning tree:** select all the switches and set **Spanning tree** to RSTP under **General → Hardware**. Or let the design checks offer *Enable RSTP*.
3. **Addresses:** in the **IP Plan** tab, click **Auto-assign (all devices)**. Every camera, controller, speaker, AP, switch SVI, server and UPS gets an address in its VLAN's subnet; Wi-Fi clients use DHCP. Check the list of anything skipped. It's usually a switch port whose mode you forgot to set.
4. **Cable labels:** in the **Patch schedule** tab, open **Auto-label**, keep the prefix *C-*, and click **Label cables**. There are about 600 cables in about 300 runs.

## Step 12: Design checks

Open the **Issues** tab. Typical findings at this point, and what to do:

| Finding | What it means | Fix |
|---|---|---|
| `poe.budget` on a services switch | Too many APs for a 24-port switch | Change the model to the 48-port PoE+ switch |
| `wifi.overlap` | Two APs on a floor share a channel | Click the fix (*Move to channel…*) |
| `tx.missing` | An SFP+ port with fibre but no module | Click *Fit 10GBASE-LR* |
| `vlan.not-carried` | A VLAN used on a floor isn't allowed on its uplink trunk | Set the uplink trunks to allow *All*, or click the fix |
| `ups.load` | A rack's UPS above 80% | Add a UPS or move load to another rack |
| `topo.loop` | A loop between switches with no spanning tree | Enable RSTP |

Click **Fix all** to apply every clear-cut fix in one step, then work through the rest. The finished template shows **No problems found**.

## Step 13: The report

Press **Ctrl+P** and click **Export…**. For this design, the report runs to about 120 pages:

- **Topology:** the whole riser diagram, followed by one detail page per floor, because the full drawing is too large to read on a single page.
- **Device inventory:** all 320 devices with model, floor, rack position and address.
- **IP addressing**, **VLANs and Wi-Fi**, and **Routing:** the plan from steps 1–5.
- **Racks and power:** an elevation of every ELV rack and both server racks, with the UPS loads.
- **Patch schedule** and **Port maps:** every run from switch to device, the material the installer needs.
- **Bill of materials:** switches, panels, optics, and cable by type with lengths to order.

## Going further

- **Resilience:** give each floor a second uplink to the other core and let RSTP block it. With spanning tree set on the switches, the loop check treats the loop as intended.
- **Recording growth:** check the recording uplink with **IT Tools → Transfer time** and **Oversubscription**, for example 18 cameras at 8 Mbit/s each on a 10G uplink.
- **Tenant isolation:** put each large tenant on its own VLAN and SSID.
- **Tidy the drawing:** select one floor's field devices and use **Arrange → Arrange selection**.
