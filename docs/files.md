# Files, autosave and settings

## Project files

A project is saved as a single `.netvis` file. It holds everything: devices, cables, areas, VLANs, subnets, Wi-Fi, racks and project details. When NetVis is installed, double-clicking a `.netvis` file opens it.

The file is readable JSON, so it works well with version control. Files saved by older versions of NetVis always open. A file from a newer version is refused with a message rather than misread.

**File → Open recent** lists the last projects you opened.

## Autosave and recovery

While there are unsaved changes, NetVis saves a recovery copy every 90 seconds. If the app closes unexpectedly, it offers to recover that copy the next time it starts. The recovery copy is deleted when you save, open another project or start a new one.

## Settings

NetVis remembers these between sessions:
- the window and panel layout;
- the theme;
- grid and snap settings;
- the report sections you chose;
- the recent-files list.

They're stored in your user profile. Uninstalling NetVis leaves them in place.

## Installing

The Windows installer (`NetVis-<version>-setup.exe`) installs NetVis to `Program Files\NetVis`. It adds Start-menu shortcuts, the `.netvis` file association, the sample projects and, optionally, a desktop shortcut. Remove it from **Settings → Apps**.

`netvis.exe` also runs on its own without installing; copy it anywhere.
