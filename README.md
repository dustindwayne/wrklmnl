# WRKLMNL — Work Liminal

![WRKLMNL wordmark](crates/terminal/assets/wordmark.png)

A native workspace for your files, shells, and connections. Point it at your desktop or any folder you already use.

| Dashboard | Terminals |
| --- | --- |
| [![Dashboard with the scratchpad and connection overview](docs/media/dashboard.png)](docs/media/dashboard.png) | [![Two local terminals open side by side](docs/media/terminals.png)](docs/media/terminals.png) |
| **Connections** | **Files** |
| [![Serial devices and a saved SSH profile](docs/media/connections.png)](docs/media/connections.png) | [![Desktop file canvas with the Projects folder open in Contents](docs/media/files.png)](docs/media/files.png) |

## Start with a folder

Open WRKLMNL, then choose a folder in **Settings → Workspace**. On Windows, launch the executable from the [latest build](https://github.com/dustindwayne/wrklmnl/releases/latest). **Use Desktop** points the app at your system Desktop.

## Dashboard

Keep notes in the scratchpad and add formatting or links to files, folders, and app pages. Notes save automatically. Connections and terminal sessions sit beside the scratchpad. Use **Arrange dashboard** to reorder Scratchpad, Connections, and Terminals or move the wider column to the other side.

## Terminals

Open local shells in tabs or split the window into panes. Add a shell in **Settings → Custom shells** to give it a working folder and startup arguments.

## Connections

Inspect detected serial devices and available driver details. Open serial, SSH, and Telnet sessions from the same screen, and save SSH profiles for destinations you revisit.

Open **File transfer** to browse local and remote folders side by side. SFTP uploads and downloads can resume after a connection drops. SSH and SFTP use trusted host keys and authenticate with a private key or the system SSH agent.

## Files

Browse your workspace as a desktop canvas. Drag items to move them, open a folder in **Contents**, or preview a file beside the canvas. Preview SVG images, rendered Markdown, and highlighted code. Lightweight text files can be edited in place. Choose a file type to sort matching items by modification date, newest first. Play supported audio and video in Contents with FFmpeg installed and `ffprobe`, `ffmpeg`, and `ffplay` on PATH. Open **Properties** for the path, name, timestamps, and permissions.

## Recent, Settings, and Shortcuts

Reopen a folder from **Recent**. Use **Settings → Layout** to reorder the dashboard and sidebar. The layout saves automatically.

Choose **Dark** or **Light** and an **Accent color** in **Settings → Appearance**. Choose a solid background or try Velvet, Ember, Prism, or Afterglow gradients. Save your theme or import one from a file. Add images to the top bar and sidebar in **Backgrounds**.

See [keyboard shortcuts](KEYBINDINGS.md) for navigation and terminal controls.

## Build

Windows is the published release. See the [distribution guide](docs/distribution.md#build-from-source) for source-build commands and prerequisites.

## Licenses

WRKLMNL has no project-wide open-source license. Third-party components keep their own terms. See the [Windows third-party notices](THIRD_PARTY_NOTICES.txt) and the [distribution guide](docs/distribution.md) for the bundled fonts, Rust standard library notices, and the corresponding source for MPL-covered dependencies.

## Source layout

- [Desktop app](crates/terminal) contains the dashboard, terminals, connections, Files, and Settings.
- [Graphics](crates/graphics), [UI components](crates/ui), and [layout parsing](crates/language) are shared workspace crates.
- [Vendored eframe patch](vendor/eframe/DTERM-PATCH.md) describes local window integration changes.
- [Third-party icon notices](assets/icons/LICENSE) and the [vendored eframe license](vendor/eframe/LICENSE-MIT) cover those bundled components.
