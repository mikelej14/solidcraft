# Native GGUF integration — clean baseline

This branch was created directly from SolidCraft's `main` branch. No files from the previous `solidcraft-agent-development` experiment were copied into it.

## Scope

Preserve the existing SolidCraft desktop CAD interface, feature tree, viewport, document engine, assistant chat, permissions, undo, and external providers/custom URL settings.

Add a local GGUF option to the **existing assistant**:

1. **Open GGUF…** — native Windows file dialog to select a model anywhere on disk.
2. **Load** — launch an application-managed, bundled llama.cpp server, bound to loopback, and automatically connect the existing OpenAI-compatible API backend.
3. **Unload** — disconnect and stop only the owned local inference process, releasing RAM/VRAM.
4. **Custom URL** — retain all existing external endpoints and credentials unchanged. Users can switch between external and local providers.
5. **Model capability** — show text/vision support accurately; only offer image input when the model and runtime support it.
6. **Runtime** — Vulkan acceleration with CPU fallback, startup/error reporting and shutdown cleanup.

## Verified source integration points

- `crates/ui-egui/src/agent_chat/backend.rs`: `ChatBackend` abstraction.
- `crates/ui-egui/src/agent_chat/api_client.rs`: existing OpenAI-compatible tool loop and MCP bridge.
- `crates/ui-egui/src/agent_chat/settings.rs`: current custom endpoint UI.
- `crates/ui-egui/src/agent_chat/agents.rs`: API connection configuration.
- `crates/ui-egui/src/agent_chat/mod.rs`: assistant module.

Do not add a second CAD editor, Python desktop application or parallel agent tool protocol.

## Implementation status

These entries distinguish **committed code** from **validated behavior**.

- [x] Clean development branch created from main (no Python prototype)
- [x] Native file picker connected to **Open GGUF…** in the existing AI settings
- [x] **Load** and **Unload** controls and UI status/error text
- [x] Owned localhost llama-server process, health-based readiness and process shutdown
- [x] Temporary OpenAI-compatible provider routed through the existing agent tool/permission/undo system
- [x] Unit test: saved external URL configurations survive local provider selection
- [x] Initial Vulkan loading with CPU-only retry on failure
- [x] SHA-verified llama.cpp Vulkan runtime archive added to Windows x64/ARM64 MSI and portable package *scripts*
- [x] Runtime extraction happens off the UI thread when a model is loaded
- [ ] Windows Rust compile and UI test suite completed successfully
- [ ] Windows packaging script run successfully for x64 and verified MSI/portable payloads
- [ ] Actual GGUF model tested through Load → chat → SolidCraft CAD tool call → Unload
- [ ] Vision/mmproj model support validated and capability gating tested
- [ ] Clean install/uninstall and external URL switching regression tested

This branch is **not yet a validated installer or release**. Do not merge until the compile and on-device acceptance tests pass. The existing custom URL configuration has not been replaced.
