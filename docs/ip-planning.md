# VLANs, subnets and Wi-Fi

## VLANs

Open the **VLANs & Wi-Fi** tab and click **Add VLAN**. Give it a tag (1–4094), a name and a purpose. Each VLAN gets a colour, which shows on switch-port outlines on the canvas.

Click a row to edit it in the Inspector. The **Used by** section lists every port, SVI, subnet and SSID that uses it. Deleting a VLAN removes every reference to it, in one step you can undo.

## Subnets and the IP plan

The **IP Plan** tab shows subnets as a tree: smaller subnets are indented under the block that contains them. Each row has a bar showing how the space is used (child subnets, assigned addresses, reserved ranges, the DHCP pool and free space).

**Add subnet** creates one. Select it to set:
- **VLAN:** binds the subnet to a VLAN; this is how auto-assign knows which addresses a port gets.
- **Gateway:** usually the first usable address.
- **DHCP:** the range, lease, DNS servers for clients, and the device that serves it.
- **Reserved ranges:** addresses auto-assign must never hand out, such as printers or a block set aside for the future.

The subnet inspector also works as a calculator: mask, wildcard, broadcast and host range. Below that it lists every address in use; click one to jump to its device.

## The VLSM planner

**VLSM planner…** divides an address block into right-sized subnets.
1. Enter the block, for example `10.50.0.0/16`.
2. Add one line per network with the hosts it needs.
3. Review the result. The planner sizes the subnets largest first and can also create:
   - the matching VLANs;
   - a gateway on the first address;
   - a DHCP scope over the upper half of each subnet.

All of it is created as a single undo step.

## Auto-assign

**Auto-assign** gives every unaddressed interface the next free address in the right subnet:

| What | How it gets its address |
|---|---|
| Device port | From the VLAN of the switch port it's cabled to: the access VLAN, or a trunk's native VLAN. Cables through patch panels and outlets are followed to the switch. The port also gets the subnet gateway as its default gateway. |
| SVI | Takes the subnet's gateway address. |
| Sub-interface | Also takes the gateway. |

It skips DHCP clients, the DHCP pool, reserved ranges and addresses already in use. When devices are selected, it only covers those. Anything it couldn't address is listed with the reason, for example *connected port has no access VLAN*.

> The usual workflow: set the switch-port modes and VLANs, then run Auto-assign once. Run it again after adding devices; it only fills the gaps.

## Wi-Fi

**Add SSID** in the VLANs & Wi-Fi tab. For each SSID, set:
- the name and the **VLAN** it bridges to;
- the **Security:** Open, OWE (Enhanced Open), WPA2/WPA3 Personal or Enterprise, or WPA2/WPA3 transition;
- the **Bands** (2.4, 5 or 6 GHz), and whether the SSID is hidden.

For each AP, set the channel and width under **Interfaces → Radio**. The design checks warn about:
- invalid channels for the band, and 40 MHz on 2.4 GHz;
- APs on the same floor or switch whose channels overlap (allowing for 40/80/160 MHz bonding);
- SSIDs whose VLAN isn't allowed on the AP's uplink.

Where a fix is clear, it offers one, such as *Move to channel 52*.

> In busy spaces (malls, offices, halls) use 20 or 40 MHz channels on 5 GHz. Narrower channels give you more of them, so neighbouring APs don't have to share.
