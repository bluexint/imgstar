import type {
  ConnectionPingResult,
  KvReadonlySnapshot,
  PluginVerificationResult,
  PreviewResult,
  UploadQueueSnapshot,
  SaveSettingsResult,
  SettingsDraft,
  SettingsSnapshot,
  UploadEvent,
  UploadEventFilter,
  UploadStartPayload,
  UploadStartResult,
  UploadRecyclePayload,
  UploadRecycleResult,
} from "@imgstar/contracts";
import type { RuntimeBridge } from "@/types/runtime";

type InvokeFn = <T>(
  command: string,
  args?: Record<string, unknown>
) => Promise<T>;

const resolveInvoke = async (): Promise<InvokeFn> => {
  const module = await import("@tauri-apps/api/core");
  return module.invoke as InvokeFn;
};

const safeInvoke = async <T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> => {
  const invoke = await resolveInvoke();
  return invoke<T>(command, args);
};

export function createTauriRuntime(): RuntimeBridge {
  return {
    startUpload(payload: UploadStartPayload): Promise<UploadStartResult> {
      return safeInvoke<UploadStartResult>("cmd_upload_start", { payload });
    },

    cancelUpload(traceId: string): Promise<void> {
      return safeInvoke<void>("cmd_upload_cancel", { traceId });
    },

    recycleUpload(payload: UploadRecyclePayload): Promise<UploadRecycleResult> {
      return safeInvoke<UploadRecycleResult>("cmd_upload_recycle", { payload });
    },

    getPreview(file): Promise<PreviewResult> {
      return safeInvoke<PreviewResult>("cmd_preview_get", { payload: file });
    },

    getUploadQueueSnapshot(): Promise<UploadQueueSnapshot> {
      return safeInvoke<UploadQueueSnapshot>("cmd_upload_queue_get_snapshot");
    },

    saveUploadQueueSnapshot(payload: UploadQueueSnapshot): Promise<void> {
      return safeInvoke<void>("cmd_upload_queue_save_snapshot", { payload });
    },

    clearUploadQueueSnapshot(): Promise<void> {
      return safeInvoke<void>("cmd_upload_queue_clear_snapshot");
    },

    releaseReservedUploadNumber(number: string): Promise<boolean> {
      return safeInvoke<boolean>("cmd_upload_release_reserved_number", { number });
    },

    verifyPlugin(pluginId: string, signerSource?: string): Promise<PluginVerificationResult> {
      return safeInvoke<PluginVerificationResult>("cmd_plugin_verify", {
        pluginId,
        signerSource
      });
    },

    getSettings(): Promise<SettingsDraft> {
      return safeInvoke<SettingsDraft>("cmd_settings_get");
    },

    getSettingsSnapshot(): Promise<SettingsSnapshot> {
      return safeInvoke<SettingsSnapshot>("cmd_settings_get_snapshot");
    },

    saveSettings(payload: SettingsDraft): Promise<SaveSettingsResult> {
      return safeInvoke<SaveSettingsResult>("cmd_settings_save", { payload });
    },

    resetApp(): Promise<SettingsSnapshot> {
      return safeInvoke<SettingsSnapshot>("cmd_settings_reset_app");
    },

    getConnectionPing(): Promise<ConnectionPingResult> {
      return safeInvoke<ConnectionPingResult>("cmd_settings_ping");
    },

    listEvents(filter: UploadEventFilter): Promise<UploadEvent[]> {
      return safeInvoke<UploadEvent[]>("cmd_logs_list", { payload: filter });
    },

    clearEvents(): Promise<void> {
      return safeInvoke<void>("cmd_logs_clear");
    },

    getKvReadonlySnapshot(): Promise<KvReadonlySnapshot> {
      return safeInvoke<KvReadonlySnapshot>("cmd_logs_kv_readonly_snapshot");
    }
  };
}
