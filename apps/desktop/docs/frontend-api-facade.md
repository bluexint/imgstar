# Frontend API Facade Freeze

## Scope

This document freezes the frontend command surface for FE-GATE handoff.
All cross-boundary DTO types come only from `@imgstar/contracts`.

## Runtime Switch

- File: `apps/desktop/src/runtime/index.ts`
- Env: `VITE_RUNTIME_MODE`
- Values:
  - `mock`: use `createMockRuntime()`
  - `tauri`: use `createTauriRuntime()`

## Facade Entry

- File: `apps/desktop/src/services/api.ts`
- Facade object: `api`
- Runtime injection for tests: `setRuntime(nextRuntime)` and `resetRuntime()`

## Command Mapping (Frozen)

| Facade method | Runtime method | Tauri command | Input DTO source | Output DTO source |
|---|---|---|---|---|
| `api.startUpload(payload)` | `startUpload(payload)` | `cmd_upload_start` | `UploadStartPayload` from contracts | `UploadStartResult` from contracts |
| `api.cancelUpload(traceId)` | `cancelUpload(traceId)` | `cmd_upload_cancel` | `traceId: string` | `void` |
| `api.getPreview(file)` | `getPreview(file)` | `cmd_preview_get` | `UploadFileRef` from contracts | `PreviewResult` from contracts |
| `api.verifyPlugin(pluginId)` | `verifyPlugin(pluginId)` | `cmd_plugin_verify` | `pluginId: string` | `PluginVerificationResult` |
| `api.getSettingsSnapshot()` | `getSettingsSnapshot()` | `cmd_settings_get_snapshot` | `void` | `SettingsSnapshot` from contracts |
| `api.saveSettings(payload)` | `saveSettings(payload)` | `cmd_settings_save` | `SettingsDraft` from contracts | `SaveSettingsResult` from contracts |
| `api.resetApp()` | `resetApp()` | `cmd_settings_reset_app` | `void` | `SettingsSnapshot` from contracts |
| `api.getConnectionPing()` | `getConnectionPing()` | `cmd_settings_ping` | `void` | `ConnectionPingResult` from contracts |
| `api.listEvents(filter)` | `listEvents(filter)` | `cmd_logs_list` | `UploadEventFilter` from contracts | `UploadEvent[]` from contracts |
| `api.clearEvents()` | `clearEvents()` | `cmd_logs_clear` | `void` | `void` |

## Error and Event Contract Source

- Error codes: `packages/contracts/src/error-codes.ts`
- Event definitions: `packages/contracts/src/events.ts`
- Upload DTO: `packages/contracts/src/upload.ts`
- Hook DTO: `packages/contracts/src/hook.ts`

## Boundary Rule

Frontend does not import Rust internals and only talks through the Facade + contracts.
