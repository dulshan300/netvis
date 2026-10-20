# Reports and exports

## PDF design report

**File → Export PDF report…** (Ctrl+P) creates a branded design report from the open project. The export dialog lets you:
- switch sections on or off (your choice is remembered);
- show IP addresses and cable lengths on the diagram;
- open the PDF when it's done.

The cover is filled from the project details: name, client, author, revision and description. To change them, click empty canvas and edit them in the Inspector.

| Section | Contents |
|---|---|
| Summary | Overview, counts, design-check result, devices by type |
| Topology | The diagram as vector graphics, with a cable legend. Large designs also get a page per building, floor or rack. |
| Device inventory | Name, type, model, location, rack position, address, ports in use, notes |
| IP addressing | Subnets with VLAN, gateway, DHCP and usage; every interface address |
| VLANs and Wi-Fi | VLAN list and SSIDs |
| Routing and services | DNS, NTP, syslog, static routes, NAT |
| Racks and power | Rack elevations, rack power against UPS, PoE budgets |
| Patch schedule | Every cable run end to end |
| Port maps | Per switch, router, firewall and panel |
| Bill of materials | Hardware, optics, cable lengths to order, racks |
| Design check | Every finding |

The contents page links to each section, and every section is a PDF bookmark. Text is selectable and diagrams stay sharp at any zoom.

> Run **Fix all** and clear the Issues tab before sending a report. The *Design check* section shows the client exactly what's left.

## Other exports

| Export | Where |
|---|---|
| Topology as SVG | **File → Export diagram as SVG…**, for presentations or editing in a drawing tool |
| Patch schedule CSV | **Patch schedule** tab → **Save CSV…** |
| Bill of materials CSV | **IT Tools → Bill of materials** |
| Calculator results | Click any value in IT Tools to copy it |
