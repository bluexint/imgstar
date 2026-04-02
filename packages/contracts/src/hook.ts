import type { ErrorCode } from "./error-codes";
import type { UploadFileRef } from "./upload";

export interface HookContext {
  file: UploadFileRef;
  traceId: string;
  stage: "pre_key" | "post_key";
  objectKey?: string;
  hookName?: string;
  hookIndex?: number;
  permissions?: Record<string, boolean>;
  config?: Record<string, unknown>;
}

export interface HookResult {
  buffer?: string;
  metadata?: Record<string, unknown>;
  error?: ErrorCode;
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  signature: string;
  signatureAlgorithm: "source_binding";
  signer: string;
  signerSource: string;
}
