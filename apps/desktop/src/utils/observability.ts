import type { UploadEvent } from "@imgstar/contracts";

export type KvEntryState = "reserved" | "active" | "recycling" | "recycled" | "failed";

export const KV_BUCKET_SIZE = 100_000;
const DEFAULT_DIGIT_COUNT = 9;
const STATE_ORDER: KvEntryState[] = ["reserved", "active", "recycling", "recycled", "failed"];

export interface KvSnapshotEntry {
  keyId: string;
  bucketIndex: number;
  bucketLabel: string;
  number?: string;
  objectKey?: string;
  fileName?: string;
  traceId: string;
  state: KvEntryState;
  updatedAt: string;
  lastEvent: UploadEvent["eventName"];
}

export interface KvBucketSummary {
  bucketIndex: number;
  label: string;
  count: number;
  stateCounts: Record<KvEntryState, number>;
  latestUpdatedAt: string;
}

export interface KvTrendPoint {
  key: string;
  label: string;
  count: number;
  stateCounts: Record<KvEntryState, number>;
}

const createStateCounts = (): Record<KvEntryState, number> => ({
  reserved: 0,
  active: 0,
  recycling: 0,
  recycled: 0,
  failed: 0
});

const formatUtc = (value: string, includeSeconds = false): string => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  const hours = String(date.getUTCHours()).padStart(2, "0");
  const minutes = String(date.getUTCMinutes()).padStart(2, "0");
  const seconds = String(date.getUTCSeconds()).padStart(2, "0");

  return includeSeconds
    ? `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`
    : `${year}-${month}-${day} ${hours}:${minutes}`;
};

const parseNumberLike = (value?: string): number | null => {
  if (!value) {
    return null;
  }

  const digits = value.replace(/\D/g, "");
  if (!digits) {
    return null;
  }

  const parsed = Number(digits);
  return Number.isFinite(parsed) ? parsed : null;
};

const extractDigits = (value?: string): string | undefined => {
  if (!value) {
    return undefined;
  }

  const digits = value.replace(/\D/g, "");
  return digits.length > 0 ? digits : undefined;
};

export const formatNumberRange = (
  bucketIndex: number,
  digitCount = DEFAULT_DIGIT_COUNT,
  bucketSize = KV_BUCKET_SIZE
): string => {
  const start = bucketIndex * bucketSize;
  const end = start + bucketSize - 1;
  const width = Math.max(digitCount, String(end).length);

  return `${String(start).padStart(width, "0")}-${String(end).padStart(width, "0")}`;
};

export const formatDataRate = (bytesPerSecond?: number | null): string => {
  if (
    bytesPerSecond === null ||
    bytesPerSecond === undefined ||
    !Number.isFinite(bytesPerSecond) ||
    bytesPerSecond < 0
  ) {
    return "—";
  }

  const units = ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"];
  let value = bytesPerSecond;
  let unitIndex = 0;

  while (value >= 1000 && unitIndex < units.length - 1) {
    value /= 1000;
    unitIndex += 1;
  }

  const precision = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  let text = value.toFixed(precision);
  while (text.includes(".") && text.endsWith("0")) {
    text = text.slice(0, -1);
  }
  if (text.endsWith(".")) {
    text = text.slice(0, -1);
  }

  return `${text} ${units[unitIndex]}`;
};

export const formatFileSize = (bytes?: number | null): string => {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) {
    return "—";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1000 && unitIndex < units.length - 1) {
    value /= 1000;
    unitIndex += 1;
  }

  const precision = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  let text = value.toFixed(precision);
  while (text.includes(".") && text.endsWith("0")) {
    text = text.slice(0, -1);
  }
  if (text.endsWith(".")) {
    text = text.slice(0, -1);
  }

  return `${text} ${units[unitIndex]}`;
};

export const formatKvTimestamp = (value: string): string => formatUtc(value, true);

export const formatKvTrendLabel = (startAt: string, endAt: string): string => {
  const start = formatUtc(startAt);
  const end = formatUtc(endAt);

  if (start === end) {
    return start;
  }

  return `${start} → ${end}`;
};

export const extractKvState = (event: UploadEvent): KvEntryState | undefined => {
  if (event.eventName === "upload:key_allocated") {
    return "reserved";
  }
  if (event.eventName === "upload:adapter_success") {
    return "active";
  }
  if (event.eventName === "upload:recycle_started") {
    return "recycling";
  }
  if (event.eventName === "upload:recycle_success") {
    return "recycled";
  }
  if (
    event.eventName === "upload:recycle_failed" ||
    event.eventName === "upload:adapter_error" ||
    event.eventName === "upload:task_failed"
  ) {
    return "failed";
  }

  return undefined;
};

export const readContextValue = (
  context: Record<string, unknown>,
  key: string
): string | undefined => {
  const value = context[key];

  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }

  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }

  if (typeof value === "boolean") {
    return value ? "true" : "false";
  }

  return undefined;
};

const resolveNumber = (event: UploadEvent): { raw?: string; numeric: number | null; digitCount: number } => {
  const rawNumber = readContextValue(event.context, "number") ?? extractDigits(readContextValue(event.context, "objectKey"));
  const parsed = parseNumberLike(rawNumber);

  return {
    raw: rawNumber,
    numeric: parsed,
    digitCount: Math.max(DEFAULT_DIGIT_COUNT, rawNumber?.length ?? 0)
  };
};

export const buildKvSnapshot = (events: UploadEvent[]): KvSnapshotEntry[] => {
  const ordered = [...events].sort(
    (left, right) => new Date(left.timestamp).getTime() - new Date(right.timestamp).getTime()
  );

  const map = new Map<string, KvSnapshotEntry>();

  for (const event of ordered) {
    const numberInfo = resolveNumber(event);
    const objectKey = readContextValue(event.context, "objectKey");
    const fileName = readContextValue(event.context, "file") ?? readContextValue(event.context, "fileName");

    if (!numberInfo.raw && !objectKey) {
      continue;
    }

    const bucketIndex = numberInfo.numeric !== null && numberInfo.numeric >= 0
      ? Math.floor(numberInfo.numeric / KV_BUCKET_SIZE)
      : 0;

    const keyId = objectKey ?? `number:${numberInfo.raw ?? "unknown"}`;
    const existing = map.get(keyId);
    const nextState = extractKvState(event);
    const bucketLabel = formatNumberRange(bucketIndex, numberInfo.digitCount);

    if (!existing) {
      map.set(keyId, {
        keyId,
        bucketIndex,
        bucketLabel,
        number: numberInfo.raw,
        objectKey,
        fileName,
        traceId: event.traceId,
        state: nextState ?? "reserved",
        updatedAt: event.timestamp,
        lastEvent: event.eventName
      });
      continue;
    }

    existing.bucketIndex = bucketIndex;
    existing.bucketLabel = bucketLabel;
    if (numberInfo.raw) {
      existing.number = numberInfo.raw;
    }
    if (objectKey) {
      existing.objectKey = objectKey;
    }
    if (fileName) {
      existing.fileName = fileName;
    }
    if (nextState) {
      existing.state = nextState;
    }

    existing.traceId = event.traceId;
    existing.updatedAt = event.timestamp;
    existing.lastEvent = event.eventName;
  }

  return Array.from(map.values()).sort(
    (left, right) => new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime()
  );
};

export const buildKvBucketSummaries = (
  entries: KvSnapshotEntry[]
): KvBucketSummary[] => {
  const buckets = new Map<number, KvBucketSummary>();

  for (const entry of entries) {
    const bucket = buckets.get(entry.bucketIndex) ?? {
      bucketIndex: entry.bucketIndex,
      label: entry.bucketLabel,
      count: 0,
      stateCounts: createStateCounts(),
      latestUpdatedAt: entry.updatedAt
    };

    bucket.count += 1;
    bucket.stateCounts[entry.state] += 1;
    if (new Date(entry.updatedAt).getTime() >= new Date(bucket.latestUpdatedAt).getTime()) {
      bucket.latestUpdatedAt = entry.updatedAt;
    }

    buckets.set(entry.bucketIndex, bucket);
  }

  return Array.from(buckets.values()).sort((left, right) => left.bucketIndex - right.bucketIndex);
};

export const buildKvTrendPoints = (
  entries: KvSnapshotEntry[],
  windowCount = 8
): KvTrendPoint[] => {
  if (entries.length === 0) {
    return [];
  }

  const ordered = [...entries].sort(
    (left, right) => new Date(left.updatedAt).getTime() - new Date(right.updatedAt).getTime()
  );
  const sliceSize = Math.max(1, Math.ceil(ordered.length / windowCount));
  const points: KvTrendPoint[] = [];

  for (let index = 0; index < ordered.length; index += sliceSize) {
    const slice = ordered.slice(index, index + sliceSize);
    if (slice.length === 0) {
      continue;
    }

    const stateCounts = createStateCounts();
    for (const entry of slice) {
      stateCounts[entry.state] += 1;
    }

    points.push({
      key: `${slice[0].updatedAt}-${slice[slice.length - 1].updatedAt}-${index}`,
      label: formatKvTrendLabel(slice[0].updatedAt, slice[slice.length - 1].updatedAt),
      count: slice.length,
      stateCounts
    });
  }

  return points.slice(-windowCount);
};

export const createStateCounter = (): Record<KvEntryState, number> => createStateCounts();

export const stateOrder = STATE_ORDER;