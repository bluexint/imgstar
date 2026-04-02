import { defineStore } from "pinia";
import type {
  PluginConfig,
  StorageTargetConfig,
  UploadFileRef,
  UploadQueueSnapshot,
  UploadTaskSnapshot,
  UploadTaskState
} from "@imgstar/contracts";
import { i18n } from "@/i18n/setup";
import { api } from "@/services/api";
import { useToastStore } from "@/stores/toastStore";
import {
  DEFAULT_IMAGE_OPTIONS,
  type ImageProcessingOptions
} from "@/types/imageProcessing";
import { createThumbnailDataUrl } from "@/utils/previewImages";
import { createTraceId } from "@/utils/trace";
import { buildUploadFileRef } from "@/utils/imagePipeline";

export interface UploadTaskRuntimeState extends UploadTaskState {
  startedAt?: number;
  completedAt?: number;
  speedBps?: number;
}

export interface StartQueuedUploadsOptions {
  pluginChain?: PluginConfig[];
  imageOptions?: ImageProcessingOptions;
}

const DEFAULT_TARGETS: StorageTargetConfig[] = [
  { id: "r2-default", label: "Cloudflare R2" }
];

let taskSeq = 0;

const buildTask = (file: UploadFileRef): UploadTaskRuntimeState => ({
  id: `task-${++taskSeq}`,
  file,
  status: "draft",
  progress: 0
});

const isImageFile = (file: UploadFileRef): boolean => {
  if (file.mimeType) {
    return file.mimeType.startsWith("image/");
  }

  return /\.(png|jpe?g|webp|gif|bmp|svg)$/i.test(file.name);
};

const isLocallyValid = (file: UploadFileRef): boolean =>
  file.size > 0 && isImageFile(file);

const resolveLocalFilePath = (file: File): string => {
  const candidate = (file as File & { path?: string }).path;
  if (typeof candidate === "string" && candidate.trim().length > 0) {
    return candidate;
  }

  return `picked/${file.name}`;
};

const resolveTaskSequenceSeed = (tasks: UploadTaskRuntimeState[]): number => {
  let nextSeed = taskSeq;

  for (const task of tasks) {
    const numericId = Number.parseInt(task.id.replace(/^task-/, ""), 10);
    if (Number.isNaN(numericId)) {
      continue;
    }

    nextSeed = Math.max(nextSeed, numericId);
  }

  return nextSeed;
};

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  const chunkSize = 0x8000;

  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...chunk);
  }

  return globalThis.btoa(binary);
};

const fileToInlineContentBase64 = async (file: File): Promise<string | undefined> => {
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    return bytesToBase64(bytes);
  } catch {
    return undefined;
  }
};

const restoreFileFromRef = (file: UploadFileRef): File | undefined => {
  if (!file.inlineContentBase64) {
    return undefined;
  }

  if (typeof globalThis.atob !== "function") {
    return undefined;
  }

  try {
    const binary = globalThis.atob(file.inlineContentBase64);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }

    return new File([bytes], file.name, {
      type: file.mimeType || "application/octet-stream"
    });
  } catch {
    return undefined;
  }
};

const toTaskSnapshot = (task: UploadTaskRuntimeState): UploadTaskSnapshot => ({
  ...task,
  file: { ...task.file }
});

const persistSnapshot = async (snapshot: UploadQueueSnapshot): Promise<void> => {
  try {
    await api.saveUploadQueueSnapshot(snapshot);
  } catch {
    // Snapshot persistence is best-effort; keep upload flow alive if storage write fails.
  }
};

const revokeThumbnailUrl = (url: string): void => {
  const revoker = globalThis.URL?.revokeObjectURL;
  if (typeof revoker !== "function") {
    return;
  }

  try {
    revoker(url);
  } catch {
    // Ignore browser-specific revoke errors.
  }
};

export const useUploadStore = defineStore("upload", {
  state: () => {
    const defaultTarget = DEFAULT_TARGETS[0];

    return {
      hydrated: false,
      tasks: [] as UploadTaskRuntimeState[],
      localFiles: {} as Record<string, File>,
      thumbnails: {} as Record<string, string>,
      persistedThumbnails: {} as Record<string, string>,
      target: defaultTarget,
      targets: DEFAULT_TARGETS,
      uploading: false
    };
  },

  getters: {
    hasQueued: (state): boolean =>
      state.tasks.some((task) => task.status === "queued"),

    hasRunning: (state): boolean =>
      state.tasks.some((task) => task.status === "running"),

    queuedCount: (state): number =>
      state.tasks.filter((task) => task.status === "queued").length,

    counters: (state): Record<string, number> => ({
      success: state.tasks.filter((task) => task.status === "success").length,
      failed: state.tasks.filter((task) => task.status === "failed").length,
      running: state.tasks.filter((task) => task.status === "running").length
    })
  },

  actions: {
    async hydrate(): Promise<void> {
      try {
        const persisted = await api.getUploadQueueSnapshot();
        const restoredTasks: UploadTaskRuntimeState[] = [];
        const releaseNumbers = new Set<string>();
        let needsPersist = false;

        for (const task of persisted.tasks) {
          const restoredTask: UploadTaskRuntimeState = {
            ...task,
            file: { ...task.file }
          };

          if (restoredTask.status === "running") {
            restoredTask.status = "failed";
            restoredTask.progress = 0;
            restoredTask.error = "UPLOAD_INTERRUPTED" as UploadTaskRuntimeState["error"];
            restoredTask.startedAt = undefined;
            restoredTask.completedAt = undefined;
            restoredTask.speedBps = undefined;
            needsPersist = true;
          }

          if (restoredTask.status !== "success" && restoredTask.number) {
            releaseNumbers.add(restoredTask.number);
          }

          restoredTasks.push(restoredTask);
        }

        this.tasks = restoredTasks;
        this.localFiles = {};
        this.thumbnails = {};
        this.persistedThumbnails = { ...persisted.thumbnails };

        const restoredTarget =
          DEFAULT_TARGETS.find((target) => target.id === persisted.targetId) ?? DEFAULT_TARGETS[0];
        this.target = restoredTarget;

        for (const task of restoredTasks) {
          const restoredFile = restoreFileFromRef(task.file);
          if (restoredFile) {
            this.localFiles[task.id] = restoredFile;
          }
        }

        taskSeq = resolveTaskSequenceSeed(restoredTasks);
        this.hydrated = true;

        if (releaseNumbers.size > 0) {
          await Promise.all(
            Array.from(releaseNumbers, async (number) => {
              try {
                await api.releaseReservedUploadNumber(number);
              } catch {
                // Ignore cleanup failures and keep the restored queue available.
              }
            })
          );
        }

        if (needsPersist) {
          await this.persistGallerySnapshot();
        }
      } catch {
        this.tasks = [];
        this.localFiles = {};
        this.thumbnails = {};
        this.persistedThumbnails = {};
        this.target = DEFAULT_TARGETS[0];
        this.hydrated = true;
      }
    },

    addFiles(files: UploadFileRef[]): string[] {
      const toastStore = useToastStore();
      const createdTaskIds: string[] = [];

      for (const file of files) {
        const task = buildTask(file);
        this.tasks.push(task);
        createdTaskIds.push(task.id);

        if (!isLocallyValid(file)) {
          toastStore.pushWarn(
            String(i18n.global.t("upload.localValidationWarning", { fileName: file.name }))
          );
        }

        task.status = "queued";
      }

      return createdTaskIds;
    },

    async addPickedFiles(files: File[]): Promise<void> {
      const refs: UploadFileRef[] = await Promise.all(
        files.map(async (file) => ({
          path: resolveLocalFilePath(file),
          name: file.name,
          size: file.size,
          mimeType: file.type || undefined,
          inlineContentBase64: await fileToInlineContentBase64(file)
        }))
      );

      const createdTaskIds = this.addFiles(refs);
      for (const [index, taskId] of createdTaskIds.entries()) {
        const file = files[index];
        if (file) {
          this.localFiles[taskId] = file;
          const thumbnailUrl = globalThis.URL?.createObjectURL ? globalThis.URL.createObjectURL(file) : undefined;
          if (thumbnailUrl) {
            this.thumbnails[taskId] = thumbnailUrl;
          }

          void this.cacheThumbnailSnapshot(taskId, file);
        }
      }

      await this.persistGallerySnapshot();
    },

    addDemoFile(name?: string): void {
      const resolvedName = name ?? `sample-${this.tasks.length + 1}.png`;
      this.addFiles([
        {
          path: `mock/${resolvedName}`,
          name: resolvedName,
          size: 128_000,
          mimeType: "image/png"
        }
      ]);
      void this.persistGallerySnapshot();
    },

    setTarget(targetId: string): void {
      if (this.hasRunning) {
        return;
      }
      const match = this.targets.find((target) => target.id === targetId);
      if (match) {
        this.target = match;
        void this.persistGallerySnapshot();
      }
    },

    async prepareTaskUploadRef(
      task: UploadTaskRuntimeState,
      activePluginChain: PluginConfig[],
      imageOptions: ImageProcessingOptions
    ): Promise<UploadFileRef> {
      const localFile = this.localFiles[task.id];
      if (!localFile) {
        return task.file;
      }

      const preparedRef = await buildUploadFileRef(
        task.file,
        localFile,
        activePluginChain,
        imageOptions
      );

      task.file = {
        ...task.file,
        path: preparedRef.path,
        size: preparedRef.size,
        mimeType: preparedRef.mimeType,
        inlineContentBase64: preparedRef.inlineContentBase64
      };

      return preparedRef;
    },

    async startQueuedUploads(options: StartQueuedUploadsOptions = {}): Promise<void> {
      if (this.uploading || !this.hasQueued) {
        return;
      }

      this.uploading = true;
      const toastStore = useToastStore();
      const activePluginChain = options.pluginChain ?? [];
      const imageOptions = options.imageOptions ?? DEFAULT_IMAGE_OPTIONS;
      let successCount = 0;

      try {
        const queuedTaskIds = this.tasks
          .filter((task) => task.status === "queued")
          .map((task) => task.id);

        for (const taskId of queuedTaskIds) {
          const task = this.tasks.find((item) => item.id === taskId);
          if (!task || task.status !== "queued") {
            continue;
          }

          const traceId = createTraceId();
          const startedAt = Date.now();
          task.traceId = traceId;
          task.status = "running";
          task.progress = 10;
          task.startedAt = startedAt;
          task.completedAt = undefined;
          task.speedBps = undefined;

          await this.persistGallerySnapshot();

          try {
            const payloadFile = await this.prepareTaskUploadRef(
              task,
              activePluginChain,
              imageOptions
            );

            const result = await api.startUpload({
              traceId,
              files: [payloadFile],
              target: this.target,
              pluginChain: activePluginChain
            });

            const fileResult = result.files?.[0];
            if (fileResult?.number) {
              task.number = fileResult.number;
            }
            if (fileResult?.objectKey) {
              task.objectKey = fileResult.objectKey;
            }

            const resolvedError =
              fileResult?.error ??
              result.error ??
              (fileResult?.status === "failed" ? "INTERNAL_ERROR" : undefined);
            const wasCancelled = this.tasks.some(
              (item) => item.id === task.id && item.status === "cancelled"
            );

            if (resolvedError === "UPLOAD_CANCELLED") {
              task.status = "cancelled";
              task.progress = 0;
              task.error = undefined;
            } else if (wasCancelled) {
              if (!task.number || !task.objectKey) {
                task.status = "cancelled";
                task.progress = 0;
                task.error = undefined;
              } else {
                const recycleResult = await api.recycleUpload({
                  number: task.number,
                  objectKey: task.objectKey,
                  fileName: task.file.name,
                  traceId: task.traceId
                });

                if (recycleResult.status === "success") {
                  task.status = "cancelled";
                  task.progress = 0;
                  task.error = undefined;
                  task.number = undefined;
                  task.objectKey = undefined;
                } else {
                  task.status = "success";
                  task.progress = 100;
                  task.error = undefined;
                  toastStore.pushWarn(
                    String(i18n.global.t("upload.failedViewDetails"))
                  );
                }
              }
            } else if (result.status === "success" && fileResult?.status !== "failed") {
              task.status = "success";
              task.progress = 100;
              task.error = undefined;
              successCount += 1;
            } else {
              task.status = "failed";
              task.progress = 0;
              task.error = resolvedError ?? "INTERNAL_ERROR";
              toastStore.pushError(
                String(i18n.global.t("upload.failedViewDetails")),
                result.traceId
              );
            }
          } catch {
            if (task.status !== "cancelled") {
              task.status = "failed";
              task.progress = 0;
              task.error = "INTERNAL_ERROR";
              toastStore.pushError(
                String(i18n.global.t("upload.serviceUnavailable")),
                task.traceId
              );
            }
          } finally {
            const finishedAt = Date.now();
            task.completedAt = finishedAt;
            const durationMs = Math.max(1, finishedAt - (task.startedAt ?? finishedAt));
            task.speedBps = (task.file.size * 1000) / durationMs;
            await this.persistGallerySnapshot();
          }
        }

        if (successCount > 0) {
          toastStore.pushInfo(
            String(i18n.global.t("upload.completed", { count: successCount }))
          );
        }
      } finally {
        this.uploading = false;
      }
    },

    async cancelTask(taskId: string): Promise<void> {
      const task = this.tasks.find((item) => item.id === taskId);
      if (!task) {
        return;
      }

      if (task.status !== "queued" && task.status !== "running") {
        return;
      }

      task.status = "cancelled";
      task.progress = 0;
      void this.persistGallerySnapshot();

      if (task.traceId) {
        try {
          await api.cancelUpload(task.traceId);
        } catch {
          // Keep local cancelled state even if backend cancel command fails.
        }
      }
    },

    retryTask(taskId: string): void {
      const task = this.tasks.find((item) => item.id === taskId);
      if (!task) {
        return;
      }

      if (task.status === "failed" || task.status === "cancelled") {
        task.status = "queued";
        task.progress = 0;
        task.error = undefined;
        task.traceId = undefined;
        task.number = undefined;
        task.objectKey = undefined;
        task.startedAt = undefined;
        task.completedAt = undefined;
        task.speedBps = undefined;
        void this.persistGallerySnapshot();
      }
    },

    async removeTask(taskId: string): Promise<void> {
      const task = this.tasks.find((item) => item.id === taskId);
      if (!task) {
        return;
      }

      if (task.status === "running") {
        return;
      }

      const toastStore = useToastStore();

      if (task.status === "success") {
        if (!task.number || !task.objectKey) {
          toastStore.pushWarn(
            String(i18n.global.t("upload.missingRecycleNumber", { fileName: task.file.name }))
          );
          return;
        }

        try {
          const recycleResult = await api.recycleUpload({
            number: task.number,
            objectKey: task.objectKey,
            fileName: task.file.name,
            traceId: task.traceId
          });

          if (recycleResult.status !== "success") {
            toastStore.pushError(String(i18n.global.t("upload.deleteFailedRecycle")), recycleResult.traceId);
            return;
          }
        } catch {
          toastStore.pushError(String(i18n.global.t("upload.deleteFailedProcess")));
          return;
        }
      }

      this.detachTask(taskId);
    },

    getThumbnail(taskId: string): string | undefined {
      return this.thumbnails[taskId] ?? this.persistedThumbnails[taskId];
    },

    getLocalFile(taskId: string): File | undefined {
      return this.localFiles[taskId];
    },

    detachTask(taskId: string): void {
      const index = this.tasks.findIndex((item) => item.id === taskId);
      if (index >= 0) {
        this.tasks.splice(index, 1);
      }

      delete this.localFiles[taskId];

      const thumbnailUrl = this.thumbnails[taskId];
      if (thumbnailUrl) {
        revokeThumbnailUrl(thumbnailUrl);
      }
      delete this.thumbnails[taskId];
      delete this.persistedThumbnails[taskId];

      void this.persistGallerySnapshot();
    },

    clearList(): void {
      if (this.hasRunning) {
        return;
      }
      Object.values(this.thumbnails).forEach((url) => revokeThumbnailUrl(url));
      this.tasks = [];
      this.localFiles = {};
      this.thumbnails = {};
      this.persistedThumbnails = {};
      void api.clearUploadQueueSnapshot();
    },

    async cacheThumbnailSnapshot(taskId: string, file: File): Promise<void> {
      const thumbnail = await createThumbnailDataUrl(file);
      if (!thumbnail) {
        return;
      }

      const taskStillExists = this.tasks.some((task) => task.id === taskId);
      if (!taskStillExists) {
        return;
      }

      this.persistedThumbnails[taskId] = thumbnail;
      await this.persistGallerySnapshot();
    },

    async persistGallerySnapshot(): Promise<void> {
      if (!this.hydrated) {
        return;
      }

      await persistSnapshot({
        tasks: this.tasks.map((task) => toTaskSnapshot(task)),
        thumbnails: { ...this.persistedThumbnails },
        targetId: this.target.id
      });
    }
  }
});
