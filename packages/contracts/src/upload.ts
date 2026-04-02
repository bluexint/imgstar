import type { ErrorCode } from "./error-codes";

export interface UploadFileRef {
  path: string;
  name: string;
  size: number;
  mimeType?: string;
  inlineContentBase64?: string;
}

export type UploadTaskStatus =
  | "draft"
  | "queued"
  | "running"
  | "success"
  | "failed"
  | "cancelled";

export interface UploadTaskState {
  id: string;
  file: UploadFileRef;
  traceId?: string;
  number?: string;
  objectKey?: string;
  progress: number;
  status: UploadTaskStatus;
  error?: ErrorCode;
}

export interface UploadTaskSnapshot extends UploadTaskState {
  startedAt?: number;
  completedAt?: number;
  speedBps?: number;
}

export interface UploadQueueSnapshot {
  tasks: UploadTaskSnapshot[];
  thumbnails: Record<string, string>;
  targetId: string;
}

export interface UploadFileResult {
  index: number;
  fileName: string;
  status: "success" | "failed";
  number?: string;
  objectKey?: string;
  error?: ErrorCode;
}

export interface StorageTargetConfig {
  id: string;
  label: string;
}

export interface PluginConfig {
  id: string;
  enabled: boolean;
  hookType: "upload" | "preview" | "security";
  stage: "pre_key" | "post_key";
  priority: number;
}

export interface UploadStartPayload {
  traceId?: string;
  files: UploadFileRef[];
  target: StorageTargetConfig;
  pluginChain: PluginConfig[];
}

export type UploadTaskRequest = UploadStartPayload;

export interface UploadStartResult {
  traceId: string;
  status: "queued" | "running" | "success" | "failed";
  error?: ErrorCode;
  files?: UploadFileResult[];
}

export interface UploadRecyclePayload {
  number: string;
  objectKey: string;
  fileName: string;
  traceId?: string;
}

export interface UploadRecycleResult {
  traceId: string;
  status: "success" | "failed";
  error?: ErrorCode;
  cachePurged: boolean;
  wafSynced: boolean;
}

export interface PreviewResult {
  fileName: string;
  hash: string;
  hashEnabled: boolean;
  hashAlgorithm?: string;
  imageDataUrl?: string;
  mimeType?: string;
}
