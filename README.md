<p align="center">
  <img src="assets/icons/app/linux/exports/hicolor/256x256/apps/io.tangencylabs.solidcraft.png" width="96" height="96" alt="SolidCraft icon" />
</p>

<h1 align="center">SolidCraft — Native GGUF Edition</h1>

<p align="center">
  <strong>Parametric 3D CAD with a built-in, locally hosted GGUF AI assistant.</strong><br />
  A Windows-focused community fork of <a href="https://github.com/tangency-labs/solidcraft">SolidCraft by Tangency Labs</a>,
  extending its existing assistant without replacing the CAD application or requiring a separate AI chat app.
</p>

<p align="center">
  <a href="https://github.com/mikelej14/solidcraft/actions/workflows/native-gguf-windows.yml"><img src="https://github.com/mikelej14/solidcraft/actions/workflows/native-gguf-windows.yml/badge.svg" alt="Windows native GGUF installer build status" /></a>
</p>

> **This is the modified fork, not the original SolidCraft distribution.**
> The desktop program is still called **SolidCraft**. The native GGUF changes are
> integrated into the **existing** AI assistant, while the original CAD UI, tool
> system, permissions, undo, and external AI providers are retained.

## What we changed

| Feature | Native GGUF edition |
| --- | --- |
| **Open GGUF…** | Choose a local `.gguf` file through the existing AI settings or assistant provider menu. |
| **Load / Unload** | Start and stop a SolidCraft-managed `llama.cpp` inference process; unloading releases the process's resources. |
| **GGUF chat-format selection** | Run llama.cpp with Jinja tool-call handling, selecting the model's embedded template instead of guessing a Qwen, Granite, or Ministral template from the filename. |
| **CAD tool compatibility check** | Before connecting the assistant, send a harmless structured function-call probe. Models that cannot produce valid OpenAI `tool_calls` are rejected with a visible error instead of silently claiming CAD capability. |
| **Bundled Windows runtime** | Windows x64 MSI and portable ZIP include a checksum-pinned Vulkan `llama.cpp` archive; no separately configured Ollama or LM Studio server is required for this option. |
| **GPU-only inference** | Loads all available model layers to Vulkan without silently retrying the entire model on CPU; a model that does not fit reports a clear error. |
| **Existing assistant integration** | The local model uses SolidCraft's OpenAI-compatible assistant backend and existing CAD tools, approval controls and undo path. |
| **Keep your other providers** | Saved external endpoints, credentials and custom URLs are not overwritten by selecting a local GGUF. |
| **Local network scope** | The managed model server binds to `127.0.0.1` (your computer only). |

**Memory policy:** This fork uses GPU-only model loading (`--gpu-layers all`), disables automatic fitting onto CPU (`--fit off`), bypasses host staging where supported (`--no-host`), and disables model-file mapping (`--load-mode none`). Models that cannot fit should fail instead of silently switching to CPU. On AMD integrated GPUs, *GPU memory and system RAM are shared physical memory* and Windows may report allocations in both categories. Disabling mapping may increase load time and does not guarantee lower peak memory in every case.

This is **local model hosting**, not a new CAD application, a separate AI UI or
a wholesale rewrite of SolidCraft's modeling engine.

### Windows installer and current validation

**Windows x64:** [Download the successful MSI + portable ZIP build](https://github.com/mikelej14/solidcraft/actions/runs/40749531287)
— sign in to GitHub if necessary and open the **Artifacts** section. Download
**`SolidCraft-Native-GGUF-Windows-x64`**, extract that artifact ZIP, then run the
`.msi` installer inside. The portable package is also included.

**Builds of the current branch:** [Windows GGUF Installer workflow](https://github.com/mikelej14/solidcraft/actions/workflows/native-gguf-windows.yml).
For the newest installer after further changes, use the latest **successful** run
and its downloadable artifact instead of an older one.

**Verified:** the Windows x64 Rust type-check, GGUF tests, WiX MSI creation,
portable ZIP creation and packaged GGUF runtime presence passed in
[the successful GitHub Actions build](https://github.com/mikelej14/solidcraft/actions/runs/40749531287).

**Compatibility check note:** This checks whether a model can emit structured tool calls,
not whether it will choose every CAD operation or dimension correctly. Not all
GGUF conversions carry suitable tool-use chat templates, and some models may fail
the check even if they can answer ordinary chat questions. The check runs after
model loading, on a worker thread, and does not change external providers.

**The GPU-only memory fix is source-committed but has not yet passed a fresh Windows MSI build or a Radeon 680M memory-profile test.**

**Not yet verified on a real user's Windows system:** clean installation,
actual GGUF model load, AI-assisted CAD tool execution, unload/reload,
Vulkan performance, and switching between local and external providers.
Vision/`mmproj` support is not presented as a completed capability.
A successful CI build does **not** by itself establish that every GGUF model or CAD
tool interaction works on your hardware.

### Using the built-in local assistant

1. Install the GGUF-enabled Windows MSI (or extract the **portable** package).
2. Open **SolidCraft → AI settings** and choose **Open GGUF…**, or use
   **Open and Load GGUF…** in the existing assistant's provider menu.
3. Select a supported text GGUF model. Choose **Load** if you selected it in settings.
4. Wait for the **GGUF loaded and ready** status, then use the normal assistant panel.
5. Select **Unload** to stop local inference when finished.

The bundled runtime runs as a child process of SolidCraft; you do not need
to start an external server manually. Use relatively small, tool-capable GGUF
models for your first tests; actual speed and tool-call reliability depend on the
model, memory, hardware and drivers.

### Platforms and upstream documentation

The native packaged GGUF workflow here currently targets **Windows x64**; the
original SolidCraft project also supports macOS, Linux and a browser app, but
its [official downloads](https://solidcraft.io/download) are **not** these
custom GGUF builds. Original documentation lives at
[docs.solidcraft.io](https://docs.solidcraft.io/).

The [GGUF integration tracker](docs/native-gguf-work-tracker.md) records
implementation details and the remaining hardware-validation work.

---

## SolidCraft CAD foundation

This fork retains the upstream SolidCraft parametric CAD functionality and its existing
tooling. The upstream project's reference tests reported 75 of 76 reference parts matching
their analytic or FreeCAD-computed volume, area, centre of mass and bounding box
([oracle](docs/oracle.md)); that figure is not a fresh, independently reproduced test of this fork.

- **Sketches and constraints:** lines, arcs, splines, text and more, solved by our own constraint solver, with parameter-driven dimensions.
- **Features:** extrude, revolve, sweep, loft, fillet, chamfer, shell, draft, holes, threads, patterns, mirror, split and combine on an editable parametric timeline.
- **Parameters and configurations** with unit-aware expressions.
- **Assemblies** with joints, motion studies, contact sets, interference and mass properties.
- **Sheet metal** with flanges, hems, fold/unfold and flat patterns, and plastic enclosure features.
- **Files:** STEP, IGES, 3MF, STL, OBJ and glTF in and out where supported, URDF export, `.sc` designs (`.solidcraft` files from earlier releases still open).
- **Desktop app, browser app, headless CLI and MCP server**, all running the same commands.

The full list is in [docs/capabilities.md](docs/capabilities.md) and [docs/features.md](docs/features.md);
status and plans are in [ROADMAP.md](ROADMAP.md). See also [what people built](docs/showcase.md),
[3D mouse support](docs/3d-mouse.md) and the [architecture](docs/architecture.md).

## Build from source

```sh
cargo run --release -p solidcraft -- --sample      # desktop app with a sample part
cargo run --release -p solidcraft-cli -- commands  # the command registry
cargo run --release -p solidcraft-cli -- run examples/bracket.json --out bracket.step
cargo xtask ci                                     # fmt, clippy, tests, icons, assets, layers, wasm
```

More commands, the Windows cross-build and the browser build are in [docs/building.md](docs/building.md).

## AI agents (MCP)

`solidcraft-cli mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server. Headless it
runs its own design session, or with `--connect` it drives the running app so you can watch.
Claude Code: `claude mcp add solidcraft -- solidcraft-cli mcp`. If a terminal doesn't find
`solidcraft-cli` (older Windows installers don't put it on `PATH`), give its full path (by default
`"C:\Program Files\SolidCraft\solidcraft-cli.exe"`). Tools, access rules and an example session:
[docs/mcp.md](docs/mcp.md).

## Scripts

A script is a JSON list of the same commands the UI runs, executed with `solidcraft-cli run`.
Format and example: [docs/scripts.md](docs/scripts.md). Pages can also be driven through the
[browser command API](docs/browser-api.md).

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) describes the original project's build and contribution
rules. For this fork's native GGUF work, see
[the integration tracker](docs/native-gguf-work-tracker.md) and
[this fork's issues](https://github.com/mikelej14/solidcraft/issues).
For upstream SolidCraft development, see
[upstream issues](https://github.com/tangency-labs/solidcraft/issues) and their
[Discord](https://discord.gg/FtU4NQNVDH).

## Original project sponsors

[![CI powered by Blacksmith](docs/sponsors/blacksmith-ci-powered.png)](https://www.blacksmith.sh)

The upstream SolidCraft project uses [Blacksmith](https://www.blacksmith.sh) for sponsored
CI minutes. This fork's Windows GGUF installer is built through GitHub Actions.

## Disclaimer

The files in this repository are provided as is, without warranty, liability or indemnity.
This community modification is not an official Tangency Labs release, and the original authors
are not responsible for changes made here. See [DISCLAIMER.md](DISCLAIMER.md).

## Licence

MIT OR Apache-2.0, at your option. Developed under the
[clean-room contribution rules](AGENTS.md); third-party code and assets retain their
documented licences and attribution.

SolidCraft is independent. Behaviour is designed from public documentation, general CAD
conventions and user feedback. Reference tests use analytic values and open-source tools.
SolidCraft is not affiliated with, sponsored by or endorsed by Autodesk, Dassault Systèmes, PTC,
Siemens or any other CAD vendor. Fusion, AutoCAD, Inventor, SolidWorks, Onshape, Creo, Blender and
Tinkercad are trademarks of their respective owners, named only to describe compatibility
(see [NOTICE](NOTICE)).
