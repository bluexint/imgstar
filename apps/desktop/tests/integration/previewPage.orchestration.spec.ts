import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import PreviewPage from "@/pages/PreviewPage.vue";
import { i18n } from "@/i18n/setup";
import { usePreviewStore } from "@/stores/previewStore";
import { useUploadStore } from "@/stores/uploadStore";

describe("PreviewPage orchestration", () => {
  it("builds preview task input in page layer before calling preview store", async () => {
    const pinia = createPinia();
    const uploadStore = useUploadStore(pinia);
    const previewStore = usePreviewStore(pinia);
    uploadStore.hydrated = true;

    const taskId = uploadStore.addFiles([
      {
        path: "picked/preview-source.png",
        name: "preview-source.png",
        size: 8192,
        mimeType: "image/png"
      }
    ])[0];

    const task = uploadStore.tasks.find((item) => item.id === taskId);
    if (!task) {
      throw new Error("task_not_created");
    }

    task.status = "success";
    task.traceId = "trace-preview";
    task.number = "000000123";
    const localFile = new File(["preview"], "preview-source.png", {
      type: "image/png"
    });
    uploadStore.localFiles[taskId] = localFile;

    const selectTaskSpy = vi
      .spyOn(previewStore, "selectTask")
      .mockResolvedValue();

    mount(PreviewPage, {
      global: {
        plugins: [pinia, i18n]
      }
    });

    await flushPromises();

    expect(selectTaskSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        id: taskId,
        file: expect.objectContaining({
          path: "picked/preview-source.png",
          name: "preview-source.png"
        }),
        localFile
      })
    );
  });
});
