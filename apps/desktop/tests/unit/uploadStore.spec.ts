import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UploadStartPayload, UploadStartResult } from "@imgstar/contracts";
import { setRuntime } from "@/services/api";
import { createMockRuntime } from "@/runtime/mock";
import { useToastStore } from "@/stores/toastStore";
import { useUploadStore } from "@/stores/uploadStore";

describe("uploadStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("uploads queued files to success", async () => {
    const uploadStore = useUploadStore();
    uploadStore.addFiles([
      {
        path: "mock/success.png",
        name: "success.png",
        size: 1024,
        mimeType: "image/png"
      }
    ]);

    await uploadStore.startQueuedUploads();

    expect(uploadStore.tasks).toHaveLength(1);
    expect(uploadStore.tasks[0].status).toBe("success");
    expect(uploadStore.tasks[0].traceId).toBeTruthy();
    expect(uploadStore.tasks[0].speedBps).toBeGreaterThan(0);
  });

  it("marks failed files and emits sticky error toast", async () => {
    const uploadStore = useUploadStore();
    const toastStore = useToastStore();

    uploadStore.addFiles([
      {
        path: "mock/fail.png",
        name: "fail.png",
        size: 1024,
        mimeType: "image/png"
      }
    ]);

    await uploadStore.startQueuedUploads();

    expect(uploadStore.tasks[0].status).toBe("failed");
    expect(toastStore.items.some((item) => item.level === "error")).toBe(true);
    expect(toastStore.items.some((item) => item.level === "info")).toBe(false);
  });

  it("starts empty and ignores browser snapshots", () => {
    window.localStorage.setItem(
      "imgstar.upload.snapshot.v1",
      JSON.stringify({
        tasks: [
          {
            id: "task-99",
            file: {
              path: "inline/sample.png",
              name: "sample.png",
              size: 1024,
              mimeType: "image/png"
            },
            traceId: "trace-99",
            number: "000000099",
            objectKey: "img/public/000000099.png",
            progress: 100,
            status: "success",
            startedAt: 1,
            completedAt: 2,
            speedBps: 1000
          }
        ],
        thumbnails: {
          "task-99": "data:image/png;base64,AAA="
        },
        targetId: "r2-default"
      })
    );

    const uploadStore = useUploadStore();

    expect(uploadStore.tasks).toHaveLength(0);
    expect(uploadStore.getThumbnail("task-99")).toBeUndefined();
  });

  it("recycles successful upload when local cancellation wins race", async () => {
    let releaseUpload: (() => void) | undefined;
    const uploadGate = new Promise<void>((resolve) => {
      releaseUpload = resolve;
    });

    const runtime = createMockRuntime();
    const recycleSpy = vi.fn(async () => ({
      traceId: "trace-recycle",
      status: "success" as const,
      cachePurged: true,
      wafSynced: true
    }));

    runtime.startUpload = vi.fn(
      async (payload: UploadStartPayload): Promise<UploadStartResult> => {
      await uploadGate;
      const input = payload.files[0];
      return {
        traceId: payload.traceId ?? "trace-upload",
          status: "success",
          files: [
            {
              index: 0,
              fileName: input.name,
              status: "success",
              number: "000000001",
              objectKey: "img/public/000000001.png"
            }
          ]
        };
      }
    );
    runtime.recycleUpload = recycleSpy;

    setRuntime(runtime);

    const uploadStore = useUploadStore();
    uploadStore.addFiles([
      {
        path: "mock/race.png",
        name: "race.png",
        size: 1024,
        mimeType: "image/png"
      }
    ]);

    const startPromise = uploadStore.startQueuedUploads();

    const firstTaskId = uploadStore.tasks[0]?.id;
    expect(firstTaskId).toBeTruthy();
    await uploadStore.cancelTask(firstTaskId as string);

    releaseUpload?.();
    await startPromise;

    expect(recycleSpy).toHaveBeenCalledTimes(1);
    expect(uploadStore.tasks[0].status).toBe("cancelled");
    expect(uploadStore.tasks[0].number).toBeUndefined();
    expect(uploadStore.tasks[0].objectKey).toBeUndefined();
  });
});
