import { describe, expect, it, vi } from "vitest";
import type { UploadEvent } from "@imgstar/contracts";
import { api, setRuntime } from "@/services/api";
import type { RuntimeBridge } from "@/types/runtime";

describe("api facade with mock runtime", () => {
  it("delegates requests to injected runtime", async () => {
    const marker = vi.fn();

    const runtime: RuntimeBridge = {
      async startUpload() {
        marker("start");
        return { traceId: "trace-1", status: "success" };
      },
      async cancelUpload() {
        marker("cancel");
      },
      async recycleUpload() {
        marker("recycle");
        return {
          traceId: "trace-1",
          status: "success",
          cachePurged: true,
          wafSynced: true
        };
      },
      async getPreview(file) {
        marker("preview");
        return { fileName: file.name, hash: "abcd", hashEnabled: true };
      },
      async getUploadQueueSnapshot() {
        marker("get-upload-queue-snapshot");
        return {
          tasks: [],
          thumbnails: {},
          targetId: "r2-default"
        };
      },
      async saveUploadQueueSnapshot() {
        marker("save-upload-queue-snapshot");
      },
      async clearUploadQueueSnapshot() {
        marker("clear-upload-queue-snapshot");
      },
      async releaseReservedUploadNumber() {
        marker("release-reserved-upload-number");
        return true;
      },
      async verifyPlugin(_pluginId: string, _signerSource?: string) {
        marker("plugin");
        return { verified: true };
      },
      async getSettings() {
        marker("get-settings");
        return {
          accessKey: "",
          secretKey: "",
          endpoint: "",
          bucket: ""
        };
      },
      async getSettingsSnapshot() {
        marker("get-settings-snapshot");
        return {
          draft: {
            accessKey: "",
            secretKey: "",
            endpoint: "",
            bucket: ""
          },
          configured: false
        };
      },
      async saveSettings() {
        marker("settings");
        return { savedAt: "2026-03-30T00:00:00.000Z" };
      },
      async resetApp() {
        marker("reset-app");
        return {
          draft: {
            accessKey: "",
            secretKey: "",
            endpoint: "",
            bucket: ""
          },
          configured: false
        };
      },
      async getConnectionPing() {
        marker("ping");
        return {
          latencyMs: 42,
          checkedAt: "2026-03-30T00:00:00.000Z"
        };
      },
      async listEvents() {
        marker("events");
        return [
          {
            traceId: "trace-1",
            timestamp: "2026-03-30T00:00:00.000Z",
            module: "upload",
            eventName: "upload:task_success",
            level: "INFO",
            status: "success",
            duration: 10,
            context: {}
          } satisfies UploadEvent
        ];
      },
      async clearEvents() {
        marker("clear-events");
      },
      async getKvReadonlySnapshot() {
        marker("kv-readonly");
        return {
          digitCount: 9,
          objects: []
        };
      }
    };

    setRuntime(runtime);

    const result = await api.startUpload({
      files: [{ path: "mock/a.png", name: "a.png", size: 1 }],
      target: { id: "r2", label: "R2" },
      pluginChain: []
    });

    const logs = await api.listEvents({});

    expect(result.status).toBe("success");
    expect(logs).toHaveLength(1);
    expect(marker).toHaveBeenCalledWith("start");
    expect(marker).toHaveBeenCalledWith("events");
  });
});
