import { describe, expect, it } from "vitest";
import {
  buildKvBucketSummaries,
  buildKvSnapshot,
  buildKvTrendPoints,
  formatDataRate,
  formatFileSize
} from "@/utils/observability";

describe("observability helpers", () => {
  it("formats upload rates and file sizes with compact units", () => {
    expect(formatDataRate(512)).toBe("512 B/s");
    expect(formatDataRate(1_500_000)).toBe("1.5 MB/s");
    expect(formatFileSize(1_500_000)).toBe("1.5 MB");
  });

  it("builds kv snapshot entries with range buckets", () => {
    const snapshot = buildKvSnapshot([
      {
        traceId: "trace-1",
        timestamp: "2026-03-30T00:00:00.000Z",
        module: "upload",
        eventName: "upload:key_allocated",
        level: "INFO",
        status: "success",
        duration: 12,
        context: {
          number: "000000000",
          objectKey: "img/public/000000000.png",
          file: "sample.png"
        }
      },
      {
        traceId: "trace-2",
        timestamp: "2026-03-30T00:01:00.000Z",
        module: "upload",
        eventName: "upload:adapter_success",
        level: "INFO",
        status: "success",
        duration: 8,
        context: {
          number: "000000000",
          objectKey: "img/public/000000000.png",
          file: "sample.png"
        }
      }
    ]);

    expect(snapshot).toHaveLength(1);
    expect(snapshot[0].state).toBe("active");
    expect(snapshot[0].bucketLabel).toBe("000000000-000099999");
  });

  it("derives bucket and trend summaries from filtered kv entries", () => {
    const snapshot = buildKvSnapshot([
      {
        traceId: "trace-1",
        timestamp: "2026-03-30T00:00:00.000Z",
        module: "upload",
        eventName: "upload:key_allocated",
        level: "INFO",
        status: "success",
        duration: 12,
        context: {
          number: "000000000",
          objectKey: "img/public/000000000.png",
          file: "sample-a.png"
        }
      },
      {
        traceId: "trace-2",
        timestamp: "2026-03-30T00:05:00.000Z",
        module: "upload",
        eventName: "upload:recycle_success",
        level: "INFO",
        status: "success",
        duration: 11,
        context: {
          number: "000100000",
          objectKey: "img/public/000100000.png",
          file: "sample-b.png"
        }
      }
    ]);

    const buckets = buildKvBucketSummaries(snapshot);
    const trendPoints = buildKvTrendPoints(snapshot, 2);

    expect(buckets).toHaveLength(2);
    expect(buckets[0].label).toBe("000000000-000099999");
    expect(trendPoints).toHaveLength(2);
    expect(trendPoints[0].count + trendPoints[1].count).toBe(2);
  });
});