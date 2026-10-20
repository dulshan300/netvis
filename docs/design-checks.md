# Design checks and fixes

NetVis checks the whole design after every change.

## Where findings appear

- **Issues tab:** the full list, filterable by severity. The tab's title shows the error and warning counts. Click a finding to select and centre the item involved.
- **Inspector:** the findings for the selected item.
- **Canvas:** red (error) or amber (warning) badges on the devices and cables involved.

| Severity | Meaning |
|---|---|
| **Error** | Won't work as designed: duplicate IP, PoE over budget, wrong fibre for the optic |
| **Warning** | Will probably cause trouble: speed mismatch, UPS above 80%, overlapping Wi-Fi channels |
| **Info** | Worth a look: unused VLAN, 2.4 GHz channel other than 1/6/11 |

## What is checked

| Area | Examples |
|---|---|
| Addressing | Duplicate IPs; addresses outside any planned subnet or in the wrong VLAN's subnet; network or broadcast addresses; wrong prefix; static addresses inside a DHCP pool; gateways outside the subnet |
| Subnets and DHCP | Gateway inside the pool; overlapping subnets; DHCP clients with no scope; pools nearly full |
| VLANs | Access VLANs that don't match across a link; native VLAN mismatches; a VLAN used on a switch but not carried by any trunk to the rest of the network; SVIs with nothing carrying their VLAN; unused VLANs |
| Wi-Fi | Invalid channels and widths; overlapping channels on the same floor or switch; SSID VLAN not allowed on an AP's uplink; open SSIDs |
| Names | Duplicate or invalid hostnames; duplicate MAC addresses; duplicate cable labels |
| Topology | Layer-2 loops between switches without spanning tree; ports used twice; devices cabled to themselves |
| Physical | Cable media, speed, length and duplex; optics; PoE budgets and classes; UPS load; rack fit and overlaps; unpatched runs |

## Fixes

Many findings have a one-click fix in the Issues tab and in the Inspector, for example:
- *Assign 10.10.40.2/24*
- *Set native 10 · Management*
- *Allow VLAN 30 on 2 trunks*
- *Move to channel 52*
- *Use Cat6A*
- *Fit 10GBASE-LR*
- *Enable RSTP on 3 switches*
- *Auto-place in rack*

**Fix all** applies every fix for errors and warnings as one undo step. It re-checks after each fix so they don't fight each other. Look through what's left: those findings need a decision, such as a bigger switch or another UPS.

> Some findings are fine to keep, such as an open guest SSID if that's what the client wants. Record the reason in the device's or project's notes so it's in the report.
