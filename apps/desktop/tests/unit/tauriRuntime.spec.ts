import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UploadStartPayload } from "@imgstar/contracts";
import { createTauriRuntime } from "@/runtime/tauri";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock
}));

describe("tauri runtime bridge", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("maps upload request to cmd_upload_start payload", async () => {
    invokeMock.mockResolvedValue({ traceId: "trace-1", status: "success" });
    const runtime = createTauriRuntime();

    const payload: UploadStartPayload = {
      files: [
        {
          path: "mock/a.png",
          name: "a.png",
          size: 1024,
          mimeType: "image/png"
        }
      ],
      target: {
        id: "r2-default",
        label: "Cloudflare R2"
      },
      pluginChain: []
    };

    const result = await runtime.startUpload(payload);

    expect(result.status).toBe("success");
    expect(invokeMock).toHaveBeenCalledWith("cmd_upload_start", {
      payload
    });
  });

  it("throws when preview IPC fails", async () => {
    invokeMock.mockRejectedValue(new Error("ipc down"));
    const runtime = createTauriRuntime();

    await expect(runtime.getPreview({
      path: "mock/a.png",
      name: "a.png",
      size: 1024,
      mimeType: "image/png"
    })).rejects.toThrow("ipc down");
  });

  it("maps recycle request to cmd_upload_recycle payload", async () => {
    invokeMock.mockResolvedValue({
      traceId: "trace-1",
      status: "success",
      cachePurged: true,
      wafSynced: true
    });
    const runtime = createTauriRuntime();

    const payload = {
      number: "000000001",
      objectKey: "img/public/000000001.png",
      fileName: "a.png",
      traceId: "trace-1"
    };

    const result = await runtime.recycleUpload(payload);

    expect(result.status).toBe("success");
    expect(invokeMock).toHaveBeenCalledWith("cmd_upload_recycle", {
      payload
    });
  });

  it("throws when settings IPC fails", async () => {
    invokeMock.mockRejectedValue(new Error("ipc down"));
    const runtime = createTauriRuntime();

    await expect(runtime.getSettings()).rejects.toThrow("ipc down");
  });

  it("throws when settings snapshot IPC fails", async () => {
    invokeMock.mockRejectedValue(new Error("ipc down"));
    const runtime = createTauriRuntime();

    await expect(runtime.getSettingsSnapshot()).rejects.toThrow("ipc down");
  });

  it("maps reset app request to cmd_settings_reset_app", async () => {
    invokeMock.mockResolvedValue({
      draft: {
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
      },
      configured: false
    });
    const runtime = createTauriRuntime();

    const snapshot = await runtime.resetApp();

    expect(snapshot.configured).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("cmd_settings_reset_app", undefined);
  });

  it("throws when logs IPC fails", async () => {
    invokeMock.mockRejectedValue(new Error("ipc down"));
    const runtime = createTauriRuntime();

    await expect(runtime.listEvents({ level: "ERROR" })).rejects.toThrow("ipc down");
  });

  it("throws when kv snapshot IPC fails", async () => {
    invokeMock.mockRejectedValue(new Error("ipc down"));
    const runtime = createTauriRuntime();

    await expect(runtime.getKvReadonlySnapshot()).rejects.toThrow("ipc down");
  });
});
