import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import type { PreviewResult } from "@imgstar/contracts";
import { usePreviewStore } from "@/stores/previewStore";

describe("previewStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("hydrates selection state but ignores persisted preview payload", () => {
    const preview: PreviewResult = {
      fileName: "sample.png",
      hash: "abcd",
      hashEnabled: true,
      hashAlgorithm: "sha256",
      imageDataUrl: "data:image/png;base64,AAA=",
      mimeType: "image/png"
    };

    window.localStorage.setItem(
      "imgstar.preview.snapshot.v1",
      JSON.stringify({
        selectedTaskId: "task-1",
        preview,
        layoutMode: "classic"
      })
    );

    const previewStore = usePreviewStore();

    expect(previewStore.selectedTaskId).toBe("task-1");
    expect(previewStore.layoutMode).toBe("classic");
    expect(previewStore.preview).toBeNull();
    expect(previewStore.status).toBe("idle");
  });
});