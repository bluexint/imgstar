export interface SettingsDraft {
  accessKey: string;
  secretKey: string;
  endpoint: string;
  bucket: string;
  zoneId?: string;
  zoneApiToken?: string;
  cdnBaseUrl?: string;
  region?: string;
  keyPattern?: string;
  digitCount?: number;
  /** @deprecated 兼容字段，已废弃。回收链路在删除与缓存清除请求完成后立即释放编号，不再等待冷却延迟 */
  reuseDelayMs?: number;
  previewHashEnabled?: boolean;
  theme?: "light" | "dark" | "system";
  language?: "zh-CN" | "en";
}

export interface SaveSettingsResult {
  savedAt: string;
}

export interface SettingsSnapshot {
  draft: SettingsDraft;
  configured: boolean;
}

export interface ConnectionPingResult {
  latencyMs: number;
  checkedAt: string;
}
