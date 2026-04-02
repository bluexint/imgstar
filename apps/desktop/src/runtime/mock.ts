import type {
  ConnectionPingResult,
  KvReadonlySnapshot,
  PluginVerificationResult,
  SaveSettingsResult,
  SettingsDraft,
  SettingsSnapshot,
  UploadEvent,
  UploadEventFilter,
  UploadEventName,
  UploadQueueSnapshot,
  UploadRecyclePayload,
  UploadRecycleResult,
  UploadStartPayload,
  UploadStartResult,
} from "@imgstar/contracts";
import type { RuntimeBridge } from "@/types/runtime";
import { createTraceId } from "@/utils/trace";

const wait = async (ms: number): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, ms));
};

const DEFAULT_SETTINGS: SettingsDraft = {
  accessKey: "",
  secretKey: "",
  endpoint: "",
  bucket: "",
  region: "auto",
  digitCount: 9,
  reuseDelayMs: 900000,
  previewHashEnabled: true,
  theme: "system",
  language: "zh-CN"
};

const DEFAULT_KEY_PATTERN_SUFFIXES = ["bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"] as const;

const defaultKeyPattern = (digitCount: number): string =>
  `^/img/public/[0-9]{${digitCount}}\\.(?:${DEFAULT_KEY_PATTERN_SUFFIXES.join("|")})$`;

const normalizeOptionalText = (value?: string | null): string | undefined => {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : undefined;
};

const normalizeTheme = (value?: SettingsDraft["theme"]): SettingsDraft["theme"] => {
  if (value === "light" || value === "dark" || value === "system") {
    return value;
  }

  return "system";
};

const normalizeLanguage = (value?: SettingsDraft["language"]): SettingsDraft["language"] => {
  if (value === "zh-CN" || value === "en") {
    return value;
  }

  return "zh-CN";
};

const normalizeSettingsDraft = (payload: SettingsDraft): SettingsDraft => {
  const digitCount = payload.digitCount ?? 9;

  return {
    accessKey: payload.accessKey.trim(),
    secretKey: payload.secretKey.trim(),
    endpoint: payload.endpoint.trim(),
    bucket: payload.bucket.trim(),
    zoneId: normalizeOptionalText(payload.zoneId),
    zoneApiToken: normalizeOptionalText(payload.zoneApiToken),
    cdnBaseUrl: normalizeOptionalText(payload.cdnBaseUrl),
    region: normalizeOptionalText(payload.region) ?? "auto",
    keyPattern: normalizeOptionalText(payload.keyPattern) ?? defaultKeyPattern(digitCount),
    digitCount,
    reuseDelayMs: payload.reuseDelayMs ?? 900000,
    previewHashEnabled: payload.previewHashEnabled ?? true,
    theme: normalizeTheme(payload.theme),
    language: normalizeLanguage(payload.language)
  };
};

const isConfigured = (settings: SettingsDraft): boolean =>
  settings.accessKey.trim().length > 0 &&
  settings.secretKey.trim().length > 0 &&
  settings.endpoint.trim().length > 0 &&
  settings.bucket.trim().length > 0 &&
  (settings.endpoint.startsWith("http://") || settings.endpoint.startsWith("https://"));

const isValidSettings = (settings: SettingsDraft): boolean => {
  if (!isConfigured(settings)) {
    return false;
  }

  if (settings.digitCount !== undefined && (settings.digitCount < 1 || settings.digitCount > 20)) {
    return false;
  }

  if (settings.reuseDelayMs !== undefined && settings.reuseDelayMs < 900000) {
    return false;
  }

  const zoneId = settings.zoneId?.trim() ?? "";
  const zoneApiToken = settings.zoneApiToken?.trim() ?? "";
  const cdnBaseUrl = settings.cdnBaseUrl?.trim() ?? "";

  const hasCloudflareFields = zoneId.length > 0 || zoneApiToken.length > 0 || cdnBaseUrl.length > 0;
  if (hasCloudflareFields) {
    if (zoneId.length === 0 || zoneApiToken.length === 0 || cdnBaseUrl.length === 0) {
      return false;
    }

    if (!cdnBaseUrl.startsWith("http://") && !cdnBaseUrl.startsWith("https://")) {
      return false;
    }
  }

  return true;
};

const fallbackHash = (name: string): string => {
  let value = 0;
  for (const char of name) {
    value = (value + char.charCodeAt(0) * 17) % 1_000_000_007;
  }
  return value.toString(16).padStart(64, "0").slice(0, 64);
};

const sha256FromText = async (value: string): Promise<string> => {
  if (!globalThis.crypto?.subtle) {
    return fallbackHash(value);
  }

  const digest = await globalThis.crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(value)
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0")
  ).join("");
};

const nowISO = (): string => new Date().toISOString();

const buildMockPing = (settings: SettingsDraft): number => {
  const seed = `${settings.endpoint}:${settings.bucket}`;
  let value = 0;
  for (const char of seed) {
    value = (value + char.charCodeAt(0) * 13) % 10_000;
  }
  return 20 + (value % 80);
};

const normalizeMockSuffix = (fileName: string): string => {
  const rawSuffix = fileName.includes(".")
    ? fileName.slice(fileName.lastIndexOf(".") + 1)
    : "bin";

  const normalized = rawSuffix
    .trim()
    .replace(/^\.+/, "")
    .replace(/[^a-zA-Z0-9]/g, "")
    .toLowerCase();

  return normalized.length > 0 ? normalized : "bin";
};

const TRUSTED_PLUGIN_IDS = new Set(["image-compress", "hidden-watermark"]);
const REVOKED_PLUGIN_IDS = new Set(["hidden-watermark-revoked"]);
const TRUSTED_SIGNER = "imgstar-official";
const TRUSTED_SIGNER_SOURCE = "imgstar-official";

export function createMockRuntime(): RuntimeBridge {
  const events: UploadEvent[] = [];
  let savedSettings: SettingsDraft = normalizeSettingsDraft({ ...DEFAULT_SETTINGS });
  const activeObjects = new Map<string, string>();
  const cancelledTraceIds = new Set<string>();
  let savedUploadQueueSnapshot: UploadQueueSnapshot = {
    tasks: [],
    thumbnails: {},
    targetId: "r2-default"
  };

  const pushEvent = (
    traceId: string,
    module: UploadEvent["module"],
    eventName: UploadEventName,
    status: UploadEvent["status"],
    level: UploadEvent["level"],
    context: Record<string, unknown>,
    duration = 0,
    errorCode?: UploadEvent["errorCode"],
    errorMessage?: string,
    stack?: string
  ): void => {
    events.unshift({
      traceId,
      timestamp: nowISO(),
      module,
      eventName,
      status,
      level,
      errorCode,
      errorMessage,
      stack,
      duration,
      context
    });
  };

  const listEvents = (filter: UploadEventFilter): UploadEvent[] => {
    return events.filter((event) => {
      if (filter.module && event.module !== filter.module) {
        return false;
      }
      if (filter.traceId && event.traceId !== filter.traceId) {
        return false;
      }
      if (filter.level && event.level !== filter.level) {
        return false;
      }
      if (filter.errorCode && event.errorCode !== filter.errorCode) {
        return false;
      }

      const eventTs = new Date(event.timestamp).getTime();
      if (filter.startAt && eventTs < new Date(filter.startAt).getTime()) {
        return false;
      }
      if (filter.endAt && eventTs > new Date(filter.endAt).getTime()) {
        return false;
      }
      return true;
    });
  };

  const startUpload = async (payload: UploadStartPayload): Promise<UploadStartResult> => {
    const traceId = payload.traceId?.trim().length
      ? payload.traceId.trim()
      : createTraceId();
    cancelledTraceIds.delete(traceId);

    const fileResults: NonNullable<UploadStartResult["files"]> = [];

    pushEvent(traceId, "upload", "upload:task_created", "success", "INFO", {
      fileCount: payload.files.length
    });

    for (const [index, file] of payload.files.entries()) {
      if (cancelledTraceIds.has(traceId)) {
        pushEvent(
          traceId,
          "upload",
          "upload:task_failed",
          "failed",
          "WARN",
          { cleanupStatus: "cancelled" },
          0,
          "UPLOAD_CANCELLED",
          "upload cancelled by user"
        );

        return {
          traceId,
          status: "failed",
          error: "UPLOAD_CANCELLED",
          files: fileResults
        };
      }

      const extension = normalizeMockSuffix(file.name);
      const number = String(index).padStart(9, "0");
      const objectKey = `img/public/${number}.${extension}`;

      pushEvent(
        traceId,
        "upload",
        "upload:hook_before_process",
        "success",
        "DEBUG",
        {
          stage: "pre_key",
          pluginCount: payload.pluginChain.length,
          file: file.name
        }
      );

      pushEvent(
        traceId,
        "upload",
        "upload:hook_after_process",
        "success",
        "DEBUG",
        {
          stage: "pre_key",
          file: file.name
        }
      );

      await wait(20);

      pushEvent(traceId, "upload", "upload:key_allocated", "success", "INFO", {
        stage: "reserved",
        file: file.name,
        number,
        objectKey
      });

      pushEvent(
        traceId,
        "upload",
        "upload:hook_before_process",
        "success",
        "DEBUG",
        {
          stage: "post_key",
          pluginCount: payload.pluginChain.length,
          file: file.name
        }
      );

      pushEvent(
        traceId,
        "upload",
        "upload:hook_after_process",
        "success",
        "DEBUG",
        {
          stage: "post_key",
          file: file.name
        }
      );

      pushEvent(traceId, "upload", "upload:adapter_start", "success", "INFO", {
        target: payload.target.id,
        file: file.name,
        objectKey
      });

      const shouldFail = /fail|error/i.test(file.name);
      if (shouldFail) {
        fileResults.push({
          index,
          fileName: file.name,
          status: "failed",
          number,
          objectKey,
          error: "ADAPTER_NETWORK_ERROR"
        });

        pushEvent(
          traceId,
          "upload",
          "upload:adapter_error",
          "failed",
          "ERROR",
          { target: payload.target.id, file: file.name, retryCount: 2 },
          60,
          "ADAPTER_NETWORK_ERROR",
          "mock adapter network error",
          "upload::adapter > ADAPTER_NETWORK_ERROR > mock adapter network error"
        );

        pushEvent(
          traceId,
          "upload",
          "upload:task_failed",
          "failed",
          "ERROR",
          { cleanupStatus: "rolled_back", file: file.name },
          60,
          "ADAPTER_NETWORK_ERROR",
          "upload failed in mock runtime",
          "upload::adapter > ADAPTER_NETWORK_ERROR > task_failed"
        );

        return {
          traceId,
          status: "failed",
          error: "ADAPTER_NETWORK_ERROR",
          files: fileResults
        };
      }

      fileResults.push({
        index,
        fileName: file.name,
        status: "success",
        number,
        objectKey
      });

      activeObjects.set(number, objectKey);

      pushEvent(traceId, "upload", "upload:waf_synced", "success", "INFO", {
        number,
        objectKey
      });

      pushEvent(traceId, "upload", "upload:adapter_success", "success", "INFO", {
        target: payload.target.id,
        file: file.name,
        objectKey,
        retryCount: 0
      });
    }
    pushEvent(traceId, "upload", "upload:task_success", "success", "INFO", {
      fileCount: payload.files.length
    });

    return {
      traceId,
      status: "success",
      files: fileResults
    };
  };

  const cancelUpload = async (traceId: string): Promise<void> => {
    const normalized = traceId.trim();
    if (normalized.length > 0) {
      cancelledTraceIds.add(normalized);
    }

    pushEvent(traceId, "upload", "upload:task_cancelled", "success", "WARN", {
      reason: "user_cancelled"
    });
    await wait(5);
  };

  const recycleUpload = async (
    payload: UploadRecyclePayload
  ): Promise<UploadRecycleResult> => {
    const traceId = payload.traceId && payload.traceId.trim().length > 0
      ? payload.traceId
      : createTraceId();
    const number = payload.number.trim();

    pushEvent(traceId, "upload", "upload:recycle_started", "success", "INFO", {
      file: payload.fileName,
      number,
      objectKey: payload.objectKey
    });

    activeObjects.delete(number);
    pushEvent(traceId, "upload", "upload:waf_synced", "success", "INFO", {
      number
    });

    await wait(10);

    pushEvent(traceId, "upload", "upload:cache_purged", "success", "INFO", {
      number
    });
    pushEvent(traceId, "upload", "upload:recycle_success", "success", "INFO", {
      number,
      cleanupStatus: "recycled_to_free"
    });

    return {
      traceId,
      status: "success",
      cachePurged: true,
      wafSynced: true
    };
  };

  const verifyPlugin = async (
    pluginId: string,
    signerSource?: string
  ): Promise<PluginVerificationResult> => {
    const traceId = createTraceId();
    const normalized = pluginId.trim();
    const normalizedSignerSource = signerSource?.trim() || TRUSTED_SIGNER_SOURCE;

    if (normalized.length === 0) {
      pushEvent(
        traceId,
        "plugin",
        "plugin:signature_rejected",
        "failed",
        "WARN",
        {
          pluginId: normalized,
          reason: "empty_plugin_id"
        },
        6,
        "SIGNATURE_VERIFY_FAILED"
      );

      return {
        verified: false,
        reason: "SIGNATURE_VERIFY_FAILED",
        signatureAlgorithm: "source_binding",
        signer: TRUSTED_SIGNER,
        signerSource: normalizedSignerSource
      };
    }

    if (REVOKED_PLUGIN_IDS.has(normalized)) {
      pushEvent(
        traceId,
        "plugin",
        "plugin:signature_revoked",
        "failed",
        "WARN",
        {
          pluginId: normalized,
          revokedAt: "2099-01-01T00:00:00.000Z"
        },
        6,
        "SIGNATURE_VERIFY_FAILED"
      );

      return {
        verified: false,
        reason: "SIGNATURE_VERIFY_FAILED",
        signatureAlgorithm: "source_binding",
        signer: TRUSTED_SIGNER,
        signerSource: normalizedSignerSource
      };
    }

    if (normalizedSignerSource !== TRUSTED_SIGNER_SOURCE) {
      pushEvent(
        traceId,
        "plugin",
        "plugin:signature_rejected",
        "failed",
        "WARN",
        {
          pluginId: normalized,
          reason: "signature_source_mismatch",
          signerSource: normalizedSignerSource
        },
        6,
        "SIGNATURE_VERIFY_FAILED"
      );

      return {
        verified: false,
        reason: "SIGNATURE_VERIFY_FAILED",
        signatureAlgorithm: "source_binding",
        signer: TRUSTED_SIGNER,
        signerSource: normalizedSignerSource
      };
    }

    if (!TRUSTED_PLUGIN_IDS.has(normalized)) {
      pushEvent(
        traceId,
        "plugin",
        "plugin:signature_rejected",
        "failed",
        "WARN",
        {
          pluginId: normalized,
          reason: "untrusted_plugin_id"
        },
        6,
        "SIGNATURE_VERIFY_FAILED"
      );

      return {
        verified: false,
        reason: "SIGNATURE_VERIFY_FAILED",
        signatureAlgorithm: "source_binding",
        signer: TRUSTED_SIGNER,
        signerSource: normalizedSignerSource
      };
    }

    pushEvent(
      traceId,
      "plugin",
      "plugin:signature_verified",
      "success",
      "INFO",
      {
        pluginId: normalized,
        signer: TRUSTED_SIGNER,
        signerSource: normalizedSignerSource,
        expiresAt: "2099-01-01T00:00:00.000Z"
      }
    );
    return {
      verified: true,
      signatureAlgorithm: "source_binding",
      signer: TRUSTED_SIGNER,
      signerSource: normalizedSignerSource
    };
  };

  const saveSettings = async (
    payload: SettingsDraft
  ): Promise<SaveSettingsResult> => {
    const normalized = normalizeSettingsDraft(payload);
    if (!isValidSettings(normalized)) {
      throw new Error("INVALID_CONFIG");
    }

    savedSettings = normalized;
    await wait(10);
    return { savedAt: nowISO() };
  };

  const getConnectionPing = async (): Promise<ConnectionPingResult> => {
    if (
      savedSettings.accessKey.trim().length === 0 ||
      savedSettings.secretKey.trim().length === 0 ||
      savedSettings.endpoint.trim().length === 0 ||
      savedSettings.bucket.trim().length === 0
    ) {
      throw new Error("INVALID_CONFIG");
    }

    await wait(8);
    return {
      latencyMs: buildMockPing(savedSettings),
      checkedAt: nowISO()
    };
  };

  return {
    startUpload,
    cancelUpload,
    recycleUpload,
    async getPreview(file) {
      await wait(15);
      const hash = await sha256FromText(`${file.path}:${file.name}:${file.size}`);
      return {
        fileName: file.name,
        hash,
        hashEnabled: true,
        hashAlgorithm: "sha256",
        imageDataUrl:
          "data:image/svg+xml;utf8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='420' height='260'%3E%3Crect width='100%25' height='100%25' fill='%23e2e8f0'/%3E%3Ctext x='50%25' y='50%25' dominant-baseline='middle' text-anchor='middle' fill='%23475569' font-size='18'%3EPreview Mock%3C/text%3E%3C/svg%3E"
      };
    },
    async getUploadQueueSnapshot(): Promise<UploadQueueSnapshot> {
      return {
        tasks: savedUploadQueueSnapshot.tasks.map((task) => ({
          ...task,
          file: { ...task.file }
        })),
        thumbnails: { ...savedUploadQueueSnapshot.thumbnails },
        targetId: savedUploadQueueSnapshot.targetId
      };
    },
    async saveUploadQueueSnapshot(payload: UploadQueueSnapshot): Promise<void> {
      savedUploadQueueSnapshot = {
        tasks: payload.tasks.map((task) => ({
          ...task,
          file: { ...task.file }
        })),
        thumbnails: { ...payload.thumbnails },
        targetId: payload.targetId
      };
    },
    async clearUploadQueueSnapshot(): Promise<void> {
      savedUploadQueueSnapshot = {
        tasks: [],
        thumbnails: {},
        targetId: "r2-default"
      };
    },
    async releaseReservedUploadNumber(_number: string): Promise<boolean> {
      return true;
    },
    verifyPlugin,
    async getSettings() {
      return { ...savedSettings };
    },
    async getSettingsSnapshot(): Promise<SettingsSnapshot> {
      return {
        draft: { ...savedSettings },
        configured: isConfigured(savedSettings)
      };
    },
    saveSettings,
    async resetApp(): Promise<SettingsSnapshot> {
      events.splice(0, events.length);
      activeObjects.clear();
      savedUploadQueueSnapshot = {
        tasks: [],
        thumbnails: {},
        targetId: "r2-default"
      };
      savedSettings = normalizeSettingsDraft({ ...DEFAULT_SETTINGS });
      await wait(8);
      return {
        draft: { ...savedSettings },
        configured: false
      };
    },
    getConnectionPing,
    async listEvents(filter) {
      return listEvents(filter);
    },
    async clearEvents() {
      events.splice(0, events.length);
    },
    async getKvReadonlySnapshot(): Promise<KvReadonlySnapshot> {
      const digitCount = Math.min(20, Math.max(1, savedSettings.digitCount ?? 9));
      return {
        digitCount,
        objects: Array.from(activeObjects.entries())
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([number, objectKey]) => ({ number, objectKey }))
      };
    }
  };
}
