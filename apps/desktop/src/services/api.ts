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
  UploadFileRef,
  UploadRecyclePayload,
  UploadRecycleResult,
  UploadStartPayload,
  UploadStartResult,
} from "@imgstar/contracts";
import { createRuntime } from "@/runtime";
import type { RuntimeBridge } from "@/types/runtime";

let runtime: RuntimeBridge = createRuntime();

export function setRuntime(nextRuntime: RuntimeBridge): void {
  runtime = nextRuntime;
}

export function resetRuntime(): void {
  runtime = createRuntime();
}

export const api = {
  startUpload(payload: UploadStartPayload): Promise<UploadStartResult> {
    return runtime.startUpload(payload);
  },

  cancelUpload(traceId: string): Promise<void> {
    return runtime.cancelUpload(traceId);
  },

  recycleUpload(payload: UploadRecyclePayload): Promise<UploadRecycleResult> {
    return runtime.recycleUpload(payload);
  },

  getPreview(file: UploadFileRef): Promise<PreviewResult> {
    return runtime.getPreview(file);
  },

  getUploadQueueSnapshot(): Promise<UploadQueueSnapshot> {
    return runtime.getUploadQueueSnapshot();
  },

  saveUploadQueueSnapshot(payload: UploadQueueSnapshot): Promise<void> {
    return runtime.saveUploadQueueSnapshot(payload);
  },

  clearUploadQueueSnapshot(): Promise<void> {
    return runtime.clearUploadQueueSnapshot();
  },

  releaseReservedUploadNumber(number: string): Promise<boolean> {
    return runtime.releaseReservedUploadNumber(number);
  },

  verifyPlugin(pluginId: string, signerSource?: string): Promise<PluginVerificationResult> {
    return runtime.verifyPlugin(pluginId, signerSource);
  },

  getSettings(): Promise<SettingsDraft> {
    return runtime.getSettings();
  },

  getSettingsSnapshot(): Promise<SettingsSnapshot> {
    return runtime.getSettingsSnapshot();
  },

  saveSettings(payload: SettingsDraft): Promise<SaveSettingsResult> {
    return runtime.saveSettings(payload);
  },

  resetApp(): Promise<SettingsSnapshot> {
    return runtime.resetApp();
  },

  getConnectionPing(): Promise<ConnectionPingResult> {
    return runtime.getConnectionPing();
  },

  listEvents(filter: UploadEventFilter): Promise<UploadEvent[]> {
    return runtime.listEvents(filter);
  },

  clearEvents(): Promise<void> {
    return runtime.clearEvents();
  },

  getKvReadonlySnapshot(): Promise<KvReadonlySnapshot> {
    return runtime.getKvReadonlySnapshot();
  }
};
