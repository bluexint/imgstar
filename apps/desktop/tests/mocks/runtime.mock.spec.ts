import { describe, expect, it } from "vitest";
import { createMockRuntime } from "@/runtime/mock";

describe("mock runtime", () => {
  it("emits success events and supports trace filtering", async () => {
    const runtime = createMockRuntime();

    const result = await runtime.startUpload({
      files: [
        {
          path: "mock/success.png",
          name: "success.png",
          size: 2048,
          mimeType: "image/png"
        }
      ],
      target: {
        id: "r2-default",
        label: "Cloudflare R2"
      },
      pluginChain: []
    });

    expect(result.status).toBe("success");

    const filtered = await runtime.listEvents({ traceId: result.traceId });
    expect(filtered.length).toBeGreaterThan(0);
    expect(filtered.every((event) => event.traceId === result.traceId)).toBe(true);
    expect(filtered.some((event) => event.eventName === "upload:task_success")).toBe(true);
  });

  it("returns failed result and allows errorCode filtering", async () => {
    const runtime = createMockRuntime();

    const result = await runtime.startUpload({
      files: [
        {
          path: "mock/fail.png",
          name: "fail.png",
          size: 1024,
          mimeType: "image/png"
        }
      ],
      target: {
        id: "r2-default",
        label: "Cloudflare R2"
      },
      pluginChain: []
    });

    expect(result.status).toBe("failed");
    expect(result.error).toBe("ADAPTER_NETWORK_ERROR");

    const errors = await runtime.listEvents({
      traceId: result.traceId,
      errorCode: "ADAPTER_NETWORK_ERROR"
    });

    expect(errors.some((event) => event.eventName === "upload:adapter_error")).toBe(true);
    expect(errors.some((event) => event.eventName === "upload:task_failed")).toBe(true);
  });

  it("allows plugin verification in mvp open mode", async () => {
    const runtime = createMockRuntime();
    const verify = await runtime.verifyPlugin("hidden-watermark");

    expect(verify.verified).toBe(true);

    const pluginEvents = await runtime.listEvents({
      module: "plugin",
      level: "INFO"
    });

    expect(pluginEvents.some((event) => event.eventName === "plugin:signature_verified")).toBe(true);
  });

  it("supports recycle flow in mock mode", async () => {
    const runtime = createMockRuntime();
    const result = await runtime.recycleUpload({
      number: "000000000",
      objectKey: "img/public/000000000.png",
      fileName: "a.png",
      traceId: "trace-r"
    });

    expect(result.status).toBe("success");
    expect(result.cachePurged).toBe(true);
    expect(result.wafSynced).toBe(true);
  });

  it("keeps readonly kv mapping after clearing logs", async () => {
    const runtime = createMockRuntime();

    await runtime.startUpload({
      files: [
        {
          path: "mock/sample.png",
          name: "sample.png",
          size: 2048,
          mimeType: "image/png"
        }
      ],
      target: {
        id: "r2-default",
        label: "Cloudflare R2"
      },
      pluginChain: []
    });

    await runtime.clearEvents();
    const snapshot = await runtime.getKvReadonlySnapshot();

    expect(snapshot.objects).toHaveLength(1);
    expect(snapshot.objects[0].number).toBe("000000000");
    expect(snapshot.objects[0].objectKey).toBe("img/public/000000000.png");
  });

  it("resets settings and kv state together", async () => {
    const runtime = createMockRuntime();

    await runtime.saveSettings({
      accessKey: "ak",
      secretKey: "sk",
      endpoint: "https://example.r2.dev",
      bucket: "demo",
      digitCount: 12,
      reuseDelayMs: 900000,
      previewHashEnabled: true,
      theme: "system",
      language: "zh-CN"
    });

    await runtime.startUpload({
      files: [
        {
          path: "mock/sample.png",
          name: "sample.png",
          size: 2048,
          mimeType: "image/png"
        }
      ],
      target: {
        id: "r2-default",
        label: "Cloudflare R2"
      },
      pluginChain: []
    });

    const snapshot = await runtime.resetApp();
    const kvSnapshot = await runtime.getKvReadonlySnapshot();

    expect(snapshot.configured).toBe(false);
    expect(kvSnapshot.objects).toHaveLength(0);
  });
});
