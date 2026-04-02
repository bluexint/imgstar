import { defineStore } from "pinia";
import type { PreviewResult, UploadFileRef } from "@imgstar/contracts";
import { api } from "@/services/api";
import { fileToDataUrl } from "@/utils/previewImages";

type PreviewStatus = "idle" | "loading" | "ready" | "error";
export type PreviewLayoutMode = "gallery" | "classic";

export interface PreviewTaskInput {
  id: string;
  file: UploadFileRef;
  localFile?: File;
}

interface PersistedPreviewSnapshot {
  selectedTaskId: string;
  layoutMode: PreviewLayoutMode;
}

const PREVIEW_SNAPSHOT_KEY = "imgstar.preview.snapshot.v1";

const loadPersistedSnapshot = (): PersistedPreviewSnapshot => {
  if (typeof globalThis.localStorage === "undefined") {
    return {
      selectedTaskId: "",
      layoutMode: "gallery"
    };
  }

  try {
    const raw = globalThis.localStorage.getItem(PREVIEW_SNAPSHOT_KEY);
    if (!raw) {
      return {
        selectedTaskId: "",
        layoutMode: "gallery"
      };
    }

    const parsed = JSON.parse(raw) as Partial<PersistedPreviewSnapshot>;
    return {
      selectedTaskId: typeof parsed.selectedTaskId === "string" ? parsed.selectedTaskId : "",
      layoutMode: parsed.layoutMode === "classic" ? "classic" : "gallery"
    };
  } catch {
    return {
      selectedTaskId: "",
      layoutMode: "gallery"
    };
  }
};

const persistSnapshot = (snapshot: PersistedPreviewSnapshot): void => {
  if (typeof globalThis.localStorage === "undefined") {
    return;
  }

  try {
    globalThis.localStorage.setItem(PREVIEW_SNAPSHOT_KEY, JSON.stringify(snapshot));
  } catch {
    // Ignore storage quota / availability issues.
  }
};

const toHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");

const buildLocalPreview = async (file: File): Promise<PreviewResult> => {
  const buffer = await file.arrayBuffer();
  if (!globalThis.crypto?.subtle) {
    throw new Error("preview_hash_not_supported");
  }

  const digest = await globalThis.crypto.subtle.digest("SHA-256", buffer);
  return {
    fileName: file.name,
    hash: toHex(new Uint8Array(digest)),
    hashEnabled: true,
    hashAlgorithm: "sha256",
    imageDataUrl: await fileToDataUrl(file),
    mimeType: file.type || "application/octet-stream"
  };
};

export const usePreviewStore = defineStore("preview", {
  state: () => {
    const persisted = loadPersistedSnapshot();

    return {
      selectedTaskId: persisted.selectedTaskId,
      preview: null as PreviewResult | null,
      layoutMode: persisted.layoutMode,
      status: "idle" as PreviewStatus,
      errorMessage: ""
    };
  },

  actions: {
    setLayoutMode(mode: PreviewLayoutMode): void {
      this.layoutMode = mode;
      this.persistSnapshot();
    },

    clearPreview(): void {
      this.selectedTaskId = "";
      this.preview = null;
      this.status = "idle";
      this.errorMessage = "";
      this.persistSnapshot();
    },

    async selectTask(task: PreviewTaskInput): Promise<void> {
      this.selectedTaskId = task.id;
      this.status = "loading";

      try {
        const localFile = task.localFile;
        if (localFile) {
          try {
            this.preview = await buildLocalPreview(localFile);
          } catch {
            this.preview = await api.getPreview(task.file);
          }
        } else {
          this.preview = await api.getPreview(task.file);
        }
        this.status = "ready";
        this.errorMessage = "";
        this.persistSnapshot();
      } catch (error) {
        this.status = "error";
        this.preview = null;
        this.errorMessage =
          error instanceof Error ? error.message : "preview_request_failed";
        this.persistSnapshot();
      }
    },

    persistSnapshot(): void {
      persistSnapshot({
        selectedTaskId: this.selectedTaskId,
        layoutMode: this.layoutMode
      });
    }
  }
});
