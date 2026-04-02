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
  UploadStartResult
} from "@imgstar/contracts";

export interface RuntimeBridge {
  startUpload(payload: UploadStartPayload): Promise<UploadStartResult>;
  cancelUpload(traceId: string): Promise<void>;
  recycleUpload(payload: UploadRecyclePayload): Promise<UploadRecycleResult>;
  getPreview(file: UploadFileRef): Promise<PreviewResult>;
  getUploadQueueSnapshot(): Promise<UploadQueueSnapshot>;
  saveUploadQueueSnapshot(payload: UploadQueueSnapshot): Promise<void>;
  clearUploadQueueSnapshot(): Promise<void>;
  releaseReservedUploadNumber(number: string): Promise<boolean>;
  verifyPlugin(pluginId: string, signerSource?: string): Promise<PluginVerificationResult>;
  getSettings(): Promise<SettingsDraft>;
  getSettingsSnapshot(): Promise<SettingsSnapshot>;
  saveSettings(payload: SettingsDraft): Promise<SaveSettingsResult>;
  resetApp(): Promise<SettingsSnapshot>;
  getConnectionPing(): Promise<ConnectionPingResult>;
  listEvents(filter: UploadEventFilter): Promise<UploadEvent[]>;
  clearEvents(): Promise<void>;
  getKvReadonlySnapshot(): Promise<KvReadonlySnapshot>;
}
