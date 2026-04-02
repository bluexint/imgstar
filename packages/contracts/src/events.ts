import type { ErrorCode } from "./error-codes";

export const UPLOAD_EVENTS = [
  "upload:task_created",
  "upload:key_allocated",
  "upload:recycle_started",
  "upload:cache_purged",
  "upload:waf_synced",
  "upload:waf_sync_error",
  "upload:recycle_success",
  "upload:recycle_failed",
  "upload:hook_before_process",
  "upload:hook_after_process",
  "upload:hook_error",
  "upload:adapter_start",
  "upload:adapter_success",
  "upload:adapter_error",
  "upload:task_success",
  "upload:task_failed",
  "upload:task_cancelled",
  "plugin:signature_verified",
  "plugin:signature_rejected",
  "plugin:signature_revoked"
] as const;

export type UploadEventName = (typeof UPLOAD_EVENTS)[number];

export type UploadEventStatus = "success" | "failed" | "skipped";

export interface UploadEvent {
  traceId: string;
  timestamp: string;
  module: "upload" | "plugin" | "storage";
  eventName: UploadEventName;
  level: "INFO" | "WARN" | "ERROR" | "DEBUG";
  status: UploadEventStatus;
  errorCode?: ErrorCode;
  errorMessage?: string;
  stack?: string;
  duration: number;
  context: Record<string, unknown>;
}

export interface UploadEventFilter {
  module?: UploadEvent["module"];
  level?: "INFO" | "WARN" | "ERROR" | "DEBUG";
  traceId?: string;
  errorCode?: ErrorCode;
  startAt?: string;
  endAt?: string;
}
