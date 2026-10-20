# Devices, ports and interfaces

Select a device to edit it in the Inspector. It has four tabs.

## General

- **Name**, vendor and notes. Notes appear in the report's device inventory, which makes them useful for things like "Stack member 2".
- **Hardware:**
  - **Model** sets the port layout, rack height, power draw and PoE budget in one step. The library is vendor-neutral, for example *48-port PoE+ access switch, 4×SFP+ (740 W)* or *Fixed dome camera (802.3af)*. Ports keep their cables and settings when their names match. If a model change would remove cabled ports, the Inspector warns you first.
  - **Rack**, **Position** and **Rack height** for rack-mounted gear.
  - **Power draw**, **PoE budget** and **Spanning tree**. Set RSTP or MSTP on switches that form loops on purpose.
- **Addresses:** a summary of every address on the device.

## Ports

The table of physical ports: name, media (RJ45, SFP, SFP+, SFP28, QSFP…), speed, PoE role and what's connected. Click a port's mode to jump to its settings on the Interfaces tab.

## Interfaces

Settings for each port:

| Setting | Meaning |
|---|---|
| **Mode** | *Host* (an end device's NIC), *Access* (one untagged VLAN), *Trunk* (tagged VLANs with a native VLAN and an allowed list), or *Routed* (a layer-3 port with its own address) |
| **Native VLAN / Allowed** | For trunks. *All* or a list of VLANs. |
| **LAG group** | Ports in the same group form one link aggregation; loop checks count them once |
| **MTU, duplex, MAC** | Optional details. MAC addresses can be in any common notation. |
| **PoE** | Whether the port supplies (PSE) or draws (PD) power, and the standard (802.3af/at/bt) |
| **Radio** | For APs: standard, band, channel, width and transmit power |
| **IP addressing** | Static addresses (with **Next free**), DHCP client, default gateway |

Below the ports you can add **logical interfaces**:
- **VLAN interface (SVI):** the gateway for a VLAN on a layer-3 switch.
- **Sub-interface:** an 802.1Q sub-interface on a router or firewall port (router-on-a-stick).
- **Loopback** and **Tunnel**.

Problems show up inline as you type: duplicate addresses, an address outside any planned subnet, the network or broadcast address, a prefix that doesn't match the subnet, or a static address inside a DHCP pool.

## Routing

Static routes (destination, next hop, distance, description), NAT rules (source PAT, static NAT, port forward) and free-text notes on dynamic routing. They appear in the report's routing section.

## Project details

Click empty canvas to edit the project itself:
- **Details:** name, client, author, company, revision and description, all shown on the report cover.
- **Network services:** domain, DNS, NTP and syslog servers.
- **Summary:** counts of everything in the design.
