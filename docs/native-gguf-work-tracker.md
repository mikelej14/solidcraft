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

## Work tracking

- [x] Clean baseline branch created from `main`
- [x] Existing assistant integration points identified
- [ ] Windows GGUF file dialog
- [ ] Local inference process management and startup readiness
- [ ] Load/Unload buttons and state
- [ ] Automatic connection through existing API backend
- [ ] External URL regression tests
- [ ] Vulkan and CPU fallback
- [ ] Model capabilities and vision
- [ ] Windows packaging
- [ ] Build, unit tests, real-model smoke tests and clean-install validation

**No installer is ready.** Check boxes only after implementation and appropriate verification.
