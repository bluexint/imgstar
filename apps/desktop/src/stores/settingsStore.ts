import { defineStore } from "pinia";
import type { SettingsDraft, SettingsSnapshot } from "@imgstar/contracts";
import { api } from "@/services/api";
import { buildWafPattern, defaultWafSuffixes } from "@/utils/waf";

type SettingsStatus = "pristine" | "dirty" | "saving" | "saved" | "error";
type EditableSettingsField =
  | "accessKey"
  | "secretKey"
  | "endpoint"
  | "bucket"
  | "zoneId"
  | "zoneApiToken"
  | "cdnBaseUrl";

const DEFAULT_REGION = "auto";
const DEFAULT_DIGIT_COUNT = 9;
const DEFAULT_REUSE_DELAY_MS = 900_000;

const normalizeDigitCount = (value?: number): number => {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_DIGIT_COUNT;
  }

  return Math.min(20, Math.max(1, Math.round(value)));
};

const normalizeReuseDelayMs = (value?: number): number => {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_REUSE_DELAY_MS;
  }

  return Math.max(DEFAULT_REUSE_DELAY_MS, Math.round(value));
};

const withInitializationDefaults = (value: SettingsDraft): SettingsDraft => {
  const digitCount = normalizeDigitCount(value.digitCount);

  return {
    ...value,
    zoneId: value.zoneId ?? "",
    zoneApiToken: value.zoneApiToken ?? "",
    cdnBaseUrl: value.cdnBaseUrl ?? "",
    region: value.region?.trim().length ? value.region : DEFAULT_REGION,
    keyPattern:
      value.keyPattern?.trim().length
        ? value.keyPattern
        : buildWafPattern(digitCount, defaultWafSuffixes),
    digitCount,
    reuseDelayMs: normalizeReuseDelayMs(value.reuseDelayMs),
    previewHashEnabled: value.previewHashEnabled ?? true,
    theme: value.theme ?? "system",
    language: value.language ?? "zh-CN"
  };
};

const EMPTY_DRAFT: SettingsDraft = {
  accessKey: "",
  secretKey: "",
  endpoint: "",
  bucket: "",
  zoneId: "",
  zoneApiToken: "",
  cdnBaseUrl: "",
  region: DEFAULT_REGION,
  digitCount: DEFAULT_DIGIT_COUNT,
  reuseDelayMs: DEFAULT_REUSE_DELAY_MS,
  previewHashEnabled: true,
  theme: "system",
  language: "zh-CN"
};

const sameDraft = (left: SettingsDraft, right: SettingsDraft): boolean => {
  const normalizedLeft = withInitializationDefaults(left);
  const normalizedRight = withInitializationDefaults(right);

  return normalizedLeft.accessKey === normalizedRight.accessKey &&
    normalizedLeft.secretKey === normalizedRight.secretKey &&
    normalizedLeft.endpoint === normalizedRight.endpoint &&
    normalizedLeft.bucket === normalizedRight.bucket &&
    (normalizedLeft.zoneId ?? "") === (normalizedRight.zoneId ?? "") &&
    (normalizedLeft.zoneApiToken ?? "") === (normalizedRight.zoneApiToken ?? "") &&
    (normalizedLeft.cdnBaseUrl ?? "") === (normalizedRight.cdnBaseUrl ?? "") &&
    (normalizedLeft.region ?? DEFAULT_REGION) === (normalizedRight.region ?? DEFAULT_REGION) &&
    (normalizedLeft.keyPattern ?? "") === (normalizedRight.keyPattern ?? "") &&
    (normalizedLeft.digitCount ?? DEFAULT_DIGIT_COUNT) === (normalizedRight.digitCount ?? DEFAULT_DIGIT_COUNT) &&
    (normalizedLeft.reuseDelayMs ?? DEFAULT_REUSE_DELAY_MS) === (normalizedRight.reuseDelayMs ?? DEFAULT_REUSE_DELAY_MS) &&
    Boolean(normalizedLeft.previewHashEnabled) === Boolean(normalizedRight.previewHashEnabled) &&
    (normalizedLeft.theme ?? "system") === (normalizedRight.theme ?? "system") &&
    (normalizedLeft.language ?? "zh-CN") === (normalizedRight.language ?? "zh-CN");
};

const applySnapshot = (snapshot: SettingsSnapshot): SettingsDraft =>
  withInitializationDefaults(snapshot.draft);

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    draft: { ...EMPTY_DRAFT },
    persisted: { ...EMPTY_DRAFT },
    persistedConfigured: false,
    status: "pristine" as SettingsStatus,
    lastSavedAt: "",
    pingMs: null as number | null,
    lastPingAt: "",
    pingRefreshing: false,
    pingError: ""
  }),

  getters: {
    isDirty: (state): boolean => state.status === "dirty",

    isConfigured: (state): boolean => state.persistedConfigured
  },

  actions: {
    async hydrate(): Promise<void> {
      try {
        const snapshot = await api.getSettingsSnapshot();
        const current = applySnapshot(snapshot);
        this.persisted = { ...current };
        this.draft = { ...current };
        this.persistedConfigured = snapshot.configured;
        this.status = snapshot.configured ? "saved" : "pristine";
        if (snapshot.configured) {
          await this.refreshPing(true);
        } else {
          this.pingMs = null;
          this.lastPingAt = "";
          this.pingError = "";
        }
      } catch {
        this.persisted = { ...EMPTY_DRAFT };
        this.draft = { ...EMPTY_DRAFT };
        this.persistedConfigured = false;
        this.status = "error";
        this.pingMs = null;
        this.lastPingAt = "";
        this.pingError = "";
      }
    },

    updateField(field: EditableSettingsField, value: string): void {
      this.draft[field] = value;
      this.status = sameDraft(this.draft, this.persisted) ? "pristine" : "dirty";
    },

    async save(): Promise<void> {
      this.status = "saving";
      try {
        const result = await api.saveSettings(this.draft);
        const snapshot = await api.getSettingsSnapshot();
        const current = applySnapshot(snapshot);
        this.persisted = { ...current };
        this.draft = { ...current };
        this.persistedConfigured = snapshot.configured;
        this.status = snapshot.configured ? "saved" : "pristine";
        this.lastSavedAt = result.savedAt;
        await this.refreshPing(true);
      } catch {
        this.status = "error";
      }
    },

    async resetApp(): Promise<void> {
      this.status = "saving";
      try {
        const snapshot = await api.resetApp();
        const current = applySnapshot(snapshot);
        this.persisted = { ...current };
        this.draft = { ...current };
        this.persistedConfigured = snapshot.configured;
        this.status = snapshot.configured ? "saved" : "pristine";
        this.lastSavedAt = "";
        this.pingMs = null;
        this.lastPingAt = "";
        this.pingError = "";
      } catch {
        this.status = "error";
      }
    },

    async refreshPing(force = false): Promise<void> {
      if (!this.isConfigured) {
        this.pingMs = null;
        this.lastPingAt = "";
        this.pingError = "";
        return;
      }

      if (this.pingRefreshing && !force) {
        return;
      }

      this.pingRefreshing = true;
      try {
        const result = await api.getConnectionPing();
        this.pingMs = result.latencyMs;
        this.lastPingAt = result.checkedAt;
        this.pingError = "";
      } catch {
        this.pingMs = null;
        this.pingError = "ADAPTER_NETWORK_ERROR";
      } finally {
        this.pingRefreshing = false;
      }
    },

    reset(): void {
      this.draft = { ...this.persisted };
      this.status = this.persistedConfigured ? "saved" : "pristine";
    }
  }
});
