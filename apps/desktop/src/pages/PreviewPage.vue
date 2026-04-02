<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { PreviewResult } from "@imgstar/contracts";
import { api } from "@/services/api";
import {
  usePreviewStore,
  type PreviewTaskInput
} from "@/stores/previewStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUploadStore } from "@/stores/uploadStore";
import type { UploadTaskRuntimeState } from "@/stores/uploadStore";

const { t } = useI18n();
const uploadStore = useUploadStore();
const previewStore = usePreviewStore();
const settingsStore = useSettingsStore();
const searchQuery = ref("");
const remotePreview = ref<PreviewResult | null>(null);
const remotePreviewStatus = ref<"idle" | "loading" | "ready" | "error">("idle");
const remotePreviewError = ref("");

const successTasks = computed(() =>
  uploadStore.tasks.filter((task) => task.status === "success")
);

const normalizedSearchQuery = computed(() =>
  searchQuery.value.trim().toLowerCase()
);

const filteredTasks = computed(() => {
  const keyword = normalizedSearchQuery.value;
  if (!keyword) {
    return successTasks.value;
  }

  return successTasks.value.filter((task) => {
    const fileName = task.file.name.toLowerCase();
    const traceId = (task.traceId ?? "").toLowerCase();
    const number = (task.number ?? "").toLowerCase();
    return (
      fileName.includes(keyword) ||
      traceId.includes(keyword) ||
      number.includes(keyword)
    );
  });
});

const selectedTaskIndex = computed(() =>
  filteredTasks.value.findIndex((task) => task.id === previewStore.selectedTaskId)
);

const selectedTask = computed<UploadTaskRuntimeState | undefined>(() => {
  const index = selectedTaskIndex.value;
  if (index < 0) {
    return undefined;
  }

  return filteredTasks.value[index];
});

const canSwitchPrevious = computed(() => selectedTaskIndex.value > 0);
const canSwitchNext = computed(
  () => selectedTaskIndex.value >= 0 && selectedTaskIndex.value < filteredTasks.value.length - 1
);
const canCompareCloud = computed(() => {
  if (!previewStore.preview || !selectedTask.value) {
    return false;
  }

  const cdnBaseUrl = settingsStore.persisted.cdnBaseUrl?.trim() ?? "";
  return cdnBaseUrl.length > 0 && Boolean(selectedTask.value.objectKey?.trim());
});

const compareCloudResult = computed<boolean | null>(() => {
  if (!previewStore.preview || !remotePreview.value) {
    return null;
  }

  return previewStore.preview.hash === remotePreview.value.hash;
});

const resetCloudPreview = (): void => {
  remotePreview.value = null;
  remotePreviewStatus.value = "idle";
  remotePreviewError.value = "";
};

const buildPublicFileUrl = (cdnBaseUrl: string, objectKey: string): string => {
  const base = cdnBaseUrl.trim().replace(/\/+$/, "");
  const key = objectKey.trim().replace(/^\/+/, "").replace(/\\/g, "/");

  if (base.length === 0) {
    return key;
  }

  if (key.length === 0) {
    return base;
  }

  return `${base}/${key}`;
};

const thumbnailOf = (taskId: string): string | undefined => {
  if (
    previewStore.selectedTaskId === taskId &&
    previewStore.preview?.imageDataUrl
  ) {
    return previewStore.preview.imageDataUrl;
  }

  return uploadStore.getThumbnail(taskId);
};

const isSelected = (taskId: string): boolean =>
  previewStore.selectedTaskId === taskId;

const toPreviewTaskInput = (task: UploadTaskRuntimeState): PreviewTaskInput => ({
  id: task.id,
  file: task.file,
  localFile: uploadStore.getLocalFile(task.id)
});

const onSelectTask = async (task: UploadTaskRuntimeState): Promise<void> => {
  resetCloudPreview();
  await previewStore.selectTask(toPreviewTaskInput(task));
};

const selectRelativeTask = async (offset: number): Promise<void> => {
  const target = filteredTasks.value[selectedTaskIndex.value + offset];
  if (!target) {
    return;
  }

  await onSelectTask(target);
};

const onPreviousTask = async (): Promise<void> => {
  if (!canSwitchPrevious.value) {
    return;
  }

  await selectRelativeTask(-1);
};

const onNextTask = async (): Promise<void> => {
  if (!canSwitchNext.value) {
    return;
  }

  await selectRelativeTask(1);
};

const onCompareCloud = async (): Promise<void> => {
  const task = selectedTask.value;
  const cdnBaseUrl = settingsStore.persisted.cdnBaseUrl?.trim() ?? "";
  const objectKey = task?.objectKey?.trim() ?? "";

  if (!task || !previewStore.preview || cdnBaseUrl.length === 0 || objectKey.length === 0) {
    return;
  }

  remotePreviewStatus.value = "loading";
  remotePreviewError.value = "";

  try {
    remotePreview.value = await api.getPreview({
      path: buildPublicFileUrl(cdnBaseUrl, objectKey),
      name: task.file.name,
      size: task.file.size,
      mimeType: task.file.mimeType
    });
    remotePreviewStatus.value = "ready";
  } catch (error) {
    remotePreview.value = null;
    remotePreviewStatus.value = "error";
    remotePreviewError.value = error instanceof Error ? error.message : "preview_request_failed";
  }
};

const clearSearch = (): void => {
  searchQuery.value = "";
};

watch(
  filteredTasks,
  (tasks) => {
    if (!uploadStore.hydrated) {
      return;
    }

    if (tasks.length === 0) {
      resetCloudPreview();
      previewStore.clearPreview();
      return;
    }

    const matched = tasks.find((item) => item.id === previewStore.selectedTaskId);
    if (!matched) {
      resetCloudPreview();
      void previewStore.selectTask(toPreviewTaskInput(tasks[0]));
      return;
    }

    if (!previewStore.preview || previewStore.preview.fileName !== matched.file.name) {
      resetCloudPreview();
      void previewStore.selectTask(toPreviewTaskInput(matched));
    }
  },
  { immediate: true }
);
</script>

<template>
  <section class="flex h-full flex-col gap-3">
    <header class="motion-panel flex flex-wrap items-center gap-2 rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-3 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.35)]">
      <div class="flex min-w-[220px] flex-1 items-center gap-3">
        <h2 class="text-base font-semibold">{{ t("preview.title") }}</h2>
        <p class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
          {{ filteredTasks.length }} / {{ successTasks.length }}
        </p>
      </div>

      <div class="flex min-w-[220px] flex-1 items-center justify-end gap-2">
        <div class="relative w-full max-w-xs">
          <input
            v-model="searchQuery"
            class="motion-field w-full rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-1.5 pr-8 text-sm text-[var(--text-main)] outline-none placeholder:text-[var(--text-placeholder)] placeholder:opacity-100 focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]"
            :placeholder="t('preview.searchPlaceholder')"
            type="text"
          />
          <button
            v-if="searchQuery"
            class="motion-press absolute right-1.5 top-1/2 grid h-6 w-6 -translate-y-1/2 place-items-center rounded-md text-sm text-[var(--text-muted)] hover:bg-[var(--bg-muted)]/65 hover:text-[var(--text-main)]"
            type="button"
            @click="clearSearch"
          >
            x
          </button>
        </div>

        <div class="inline-flex overflow-hidden rounded-lg border border-[var(--border-muted)]">
          <button
            :class="[
              'motion-press grid h-9 w-9 place-items-center',
              previewStore.layoutMode === 'gallery'
                ? 'bg-[var(--accent)] text-slate-900 shadow-[inset_0_0_0_1px_rgba(15,23,42,0.15)]'
                : 'bg-[var(--bg-panel)] text-[var(--text-main)] hover:bg-[var(--bg-muted)]/60'
            ]"
            type="button"
            :title="t('preview.layoutGallery')"
            :aria-label="t('preview.layoutGallery')"
            :aria-pressed="previewStore.layoutMode === 'gallery'"
            @click="previewStore.setLayoutMode('gallery')"
          >
            <svg
              class="h-4 w-4"
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.8"
              viewBox="0 0 24 24"
            >
              <path d="M4.5 4.5h6v6h-6z" />
              <path d="M13.5 4.5h6v6h-6z" />
              <path d="M4.5 13.5h6v6h-6z" />
              <path d="M13.5 13.5h6v6h-6z" />
            </svg>
            <span class="sr-only">{{ t("preview.layoutGallery") }}</span>
          </button>

          <button
            :class="[
              'motion-press grid h-9 w-9 place-items-center',
              previewStore.layoutMode === 'classic'
                ? 'bg-[var(--accent)] text-slate-900 shadow-[inset_0_0_0_1px_rgba(15,23,42,0.15)]'
                : 'bg-[var(--bg-panel)] text-[var(--text-main)] hover:bg-[var(--bg-muted)]/60'
            ]"
            type="button"
            :title="t('preview.layoutClassic')"
            :aria-label="t('preview.layoutClassic')"
            :aria-pressed="previewStore.layoutMode === 'classic'"
            @click="previewStore.setLayoutMode('classic')"
          >
            <svg
              class="h-4 w-4"
              fill="none"
              stroke="currentColor"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="1.8"
              viewBox="0 0 24 24"
            >
              <path d="M4 6h16" />
              <path d="M4 12h16" />
              <path d="M4 18h16" />
              <path d="M7 4v4" />
              <path d="M12 10v4" />
              <path d="M17 16v4" />
            </svg>
            <span class="sr-only">{{ t("preview.layoutClassic") }}</span>
          </button>
        </div>
      </div>
    </header>

    <section
      v-if="previewStore.layoutMode === 'gallery'"
      class="grid h-full min-h-0 grid-cols-1 gap-4 xl:grid-cols-[1fr_360px]"
    >
      <div class="motion-panel overflow-hidden rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
        <div
          v-if="filteredTasks.length > 0"
          class="grid max-h-[640px] gap-3 overflow-auto p-3"
          :style="{ gridTemplateColumns: 'repeat(auto-fit, minmax(9rem, 1fr))' }"
        >
          <button
            v-for="task in filteredTasks"
            :key="task.id"
            :class="[
              'group relative aspect-square overflow-hidden rounded-xl border transition duration-150',
              isSelected(task.id)
                ? 'border-[var(--accent)] ring-2 ring-[var(--accent)]/30'
                : 'border-[var(--border-muted)] hover:border-[var(--accent)]/60'
            ]"
            type="button"
            @click="onSelectTask(task)"
          >
            <img
              v-if="thumbnailOf(task.id)"
              :src="thumbnailOf(task.id)"
              :alt="task.file.name"
              class="h-full w-full object-cover transition duration-200 group-hover:scale-[1.02]"
            />
            <div
              v-else
              class="grid h-full w-full place-items-center bg-[var(--bg-muted)]/50 text-xs uppercase tracking-wider text-[var(--text-muted)]"
            >
              {{ t("preview.noImage") }}
            </div>
            <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-slate-900/75 to-transparent px-2 pb-2 pt-6 text-left text-xs text-slate-100">
              <div class="truncate">{{ task.file.name }}</div>
            </div>
          </button>
        </div>
        <p v-else class="px-4 py-6 text-sm text-[var(--text-muted)]">
          {{ successTasks.length > 0 ? t("preview.searchEmpty") : t("preview.empty") }}
        </p>
      </div>

      <div class="motion-panel rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
        <div v-if="previewStore.status === 'loading'" class="space-y-3">
          <div class="h-5 w-40 rounded-md motion-loading-shimmer"></div>
          <div class="h-3 w-56 rounded-md motion-loading-shimmer"></div>
          <div class="h-[320px] rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] motion-loading-shimmer"></div>
        </div>

        <template v-else-if="previewStore.preview">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h3 class="text-base font-semibold">{{ previewStore.preview.fileName }}</h3>
              <p class="mt-2 break-all text-sm text-[var(--text-muted)]">
                {{ t("preview.hash") }}
                <span v-if="previewStore.preview.hashAlgorithm">({{ previewStore.preview.hashAlgorithm }})</span>
                : {{ previewStore.preview.hash }}
              </p>
            </div>

            <div class="flex flex-wrap items-center gap-2 text-xs">
              <button
                :disabled="!canSwitchPrevious"
                class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-2.5 py-1.5 text-[var(--text-main)] hover:bg-[var(--bg-muted)]/65 disabled:cursor-not-allowed disabled:opacity-40"
                type="button"
                @click="onPreviousTask"
              >
                {{ t("preview.switchPrevious") }}
              </button>

              <button
                :disabled="!canSwitchNext"
                class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-2.5 py-1.5 text-[var(--text-main)] hover:bg-[var(--bg-muted)]/65 disabled:cursor-not-allowed disabled:opacity-40"
                type="button"
                @click="onNextTask"
              >
                {{ t("preview.switchNext") }}
              </button>

              <button
                :disabled="!canCompareCloud || remotePreviewStatus === 'loading'"
                class="motion-press rounded-lg bg-[var(--accent)] px-3 py-1.5 text-white hover:bg-[var(--accent-strong)] disabled:cursor-not-allowed disabled:opacity-45"
                type="button"
                @click="onCompareCloud"
              >
                {{ t("preview.compareCloud") }}
              </button>

              <span class="rounded-lg bg-[var(--bg-muted)]/65 px-2.5 py-1.5 text-[var(--text-muted)]">
                {{ selectedTaskIndex >= 0 ? `${selectedTaskIndex + 1} / ${filteredTasks.length}` : "0 / 0" }}
              </span>
            </div>
          </div>

          <div class="mt-3 grid gap-3 xl:grid-cols-2">
            <article class="overflow-hidden rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35">
              <header class="border-b border-[var(--border-muted)] px-3 py-2 text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
                {{ t("preview.localPreview") }}
              </header>

              <div class="p-2">
                <img
                  v-if="previewStore.preview.imageDataUrl"
                  :src="previewStore.preview.imageDataUrl"
                  :alt="previewStore.preview.fileName"
                  class="max-h-[420px] w-full rounded-lg object-contain"
                />
                <p v-else class="py-8 text-center text-sm text-[var(--text-muted)]">{{ t("preview.noPreviewImage") }}</p>
              </div>
            </article>

            <article class="overflow-hidden rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35">
              <header class="border-b border-[var(--border-muted)] px-3 py-2 text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
                {{ t("preview.cloudPreview") }}
              </header>

              <div class="p-2">
                <div
                  v-if="remotePreviewStatus === 'loading'"
                  class="grid min-h-[220px] place-items-center rounded-lg border border-dashed border-[var(--border-muted)] bg-[var(--bg-main)] text-sm text-[var(--text-muted)]"
                >
                  {{ t("preview.cloudPreviewLoading") }}
                </div>

                <template v-else-if="remotePreview">
                  <img
                    v-if="remotePreview.imageDataUrl"
                    :src="remotePreview.imageDataUrl"
                    :alt="remotePreview.fileName"
                    class="max-h-[420px] w-full rounded-lg object-contain"
                  />
                  <p v-else class="py-8 text-center text-sm text-[var(--text-muted)]">{{ t("preview.noPreviewImage") }}</p>
                </template>

                <div
                  v-else
                  class="grid min-h-[220px] place-items-center rounded-lg border border-dashed border-[var(--border-muted)] bg-[var(--bg-main)] px-4 text-center text-sm text-[var(--text-muted)]"
                >
                  <p v-if="remotePreviewStatus === 'error'">{{ remotePreviewError }}</p>
                  <p v-else>{{ t("preview.cloudPreviewHint") }}</p>
                </div>
              </div>
            </article>
          </div>

          <p
            v-if="compareCloudResult !== null"
            :class="compareCloudResult ? 'text-emerald-600' : 'text-[var(--state-failed-text)]'"
            class="mt-3 text-sm font-medium"
          >
            {{ compareCloudResult ? t("preview.compareMatch") : t("preview.compareMismatch") }}
          </p>
        </template>

        <p v-else-if="previewStore.status === 'error'" class="text-sm text-[var(--state-failed-text)]">{{ previewStore.errorMessage }}</p>

        <p v-else class="text-sm text-[var(--text-muted)]">{{ t("preview.empty") }}</p>
      </div>
    </section>

    <section v-else class="grid h-full min-h-0 grid-cols-[280px_1fr] gap-4">
      <div class="motion-panel overflow-hidden rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
        <header class="border-b border-[var(--border-muted)] px-3 py-2 text-sm font-semibold">
          {{ t("preview.title") }}
        </header>
        <ul v-if="filteredTasks.length > 0" class="max-h-[520px] overflow-auto">
          <li v-for="task in filteredTasks" :key="task.id">
            <button
              :class="[
                'w-full border-b border-[var(--border-muted)] px-3 py-2 text-left text-sm transition duration-150 hover:bg-[var(--bg-muted)]/60',
                isSelected(task.id) ? 'bg-[var(--bg-muted)]/55' : ''
              ]"
              type="button"
              @click="onSelectTask(task)"
            >
              <div>{{ task.file.name }}</div>
              <div class="text-xs text-[var(--text-muted)]">{{ task.traceId }}</div>
            </button>
          </li>
        </ul>
        <p v-else class="px-3 py-4 text-sm text-[var(--text-muted)]">
          {{ successTasks.length > 0 ? t("preview.searchEmpty") : t("preview.empty") }}
        </p>
      </div>

      <div class="motion-panel rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
        <div v-if="previewStore.status === 'loading'" class="space-y-3">
          <div class="h-5 w-40 rounded-md motion-loading-shimmer"></div>
          <div class="h-3 w-56 rounded-md motion-loading-shimmer"></div>
          <div class="h-[320px] rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] motion-loading-shimmer"></div>
        </div>

        <template v-else-if="previewStore.preview">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h3 class="text-base font-semibold">{{ previewStore.preview.fileName }}</h3>
              <p class="mt-2 break-all text-sm text-[var(--text-muted)]">
                {{ t("preview.hash") }}
                <span v-if="previewStore.preview.hashAlgorithm">({{ previewStore.preview.hashAlgorithm }})</span>
                : {{ previewStore.preview.hash }}
              </p>
            </div>

            <div class="flex flex-wrap items-center gap-2 text-xs">
              <button
                :disabled="!canSwitchPrevious"
                class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-2.5 py-1.5 text-[var(--text-main)] hover:bg-[var(--bg-muted)]/65 disabled:cursor-not-allowed disabled:opacity-40"
                type="button"
                @click="onPreviousTask"
              >
                {{ t("preview.switchPrevious") }}
              </button>

              <button
                :disabled="!canSwitchNext"
                class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-2.5 py-1.5 text-[var(--text-main)] hover:bg-[var(--bg-muted)]/65 disabled:cursor-not-allowed disabled:opacity-40"
                type="button"
                @click="onNextTask"
              >
                {{ t("preview.switchNext") }}
              </button>

              <button
                :disabled="!canCompareCloud || remotePreviewStatus === 'loading'"
                class="motion-press rounded-lg bg-[var(--accent)] px-3 py-1.5 text-white hover:bg-[var(--accent-strong)] disabled:cursor-not-allowed disabled:opacity-45"
                type="button"
                @click="onCompareCloud"
              >
                {{ t("preview.compareCloud") }}
              </button>

              <span class="rounded-lg bg-[var(--bg-muted)]/65 px-2.5 py-1.5 text-[var(--text-muted)]">
                {{ selectedTaskIndex >= 0 ? `${selectedTaskIndex + 1} / ${filteredTasks.length}` : "0 / 0" }}
              </span>
            </div>
          </div>

          <div class="mt-3 grid gap-3 xl:grid-cols-2">
            <article class="overflow-hidden rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35">
              <header class="border-b border-[var(--border-muted)] px-3 py-2 text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
                {{ t("preview.localPreview") }}
              </header>

              <div class="p-2">
                <img
                  v-if="previewStore.preview.imageDataUrl"
                  :src="previewStore.preview.imageDataUrl"
                  :alt="previewStore.preview.fileName"
                  class="max-h-[420px] w-full rounded-lg object-contain"
                />
                <p v-else class="py-8 text-center text-sm text-[var(--text-muted)]">{{ t("preview.noPreviewImage") }}</p>
              </div>
            </article>

            <article class="overflow-hidden rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35">
              <header class="border-b border-[var(--border-muted)] px-3 py-2 text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
                {{ t("preview.cloudPreview") }}
              </header>

              <div class="p-2">
                <div
                  v-if="remotePreviewStatus === 'loading'"
                  class="grid min-h-[220px] place-items-center rounded-lg border border-dashed border-[var(--border-muted)] bg-[var(--bg-main)] text-sm text-[var(--text-muted)]"
                >
                  {{ t("preview.cloudPreviewLoading") }}
                </div>

                <template v-else-if="remotePreview">
                  <img
                    v-if="remotePreview.imageDataUrl"
                    :src="remotePreview.imageDataUrl"
                    :alt="remotePreview.fileName"
                    class="max-h-[420px] w-full rounded-lg object-contain"
                  />
                  <p v-else class="py-8 text-center text-sm text-[var(--text-muted)]">{{ t("preview.noPreviewImage") }}</p>
                </template>

                <div
                  v-else
                  class="grid min-h-[220px] place-items-center rounded-lg border border-dashed border-[var(--border-muted)] bg-[var(--bg-main)] px-4 text-center text-sm text-[var(--text-muted)]"
                >
                  <p v-if="remotePreviewStatus === 'error'">{{ remotePreviewError }}</p>
                  <p v-else>{{ t("preview.cloudPreviewHint") }}</p>
                </div>
              </div>
            </article>
          </div>

          <p
            v-if="compareCloudResult !== null"
            :class="compareCloudResult ? 'text-emerald-600' : 'text-[var(--state-failed-text)]'"
            class="mt-3 text-sm font-medium"
          >
            {{ compareCloudResult ? t("preview.compareMatch") : t("preview.compareMismatch") }}
          </p>
        </template>

        <p v-else-if="previewStore.status === 'error'" class="text-sm text-[var(--state-failed-text)]">{{ previewStore.errorMessage }}</p>

        <p v-else class="text-sm text-[var(--text-muted)]">{{ t("preview.empty") }}</p>
      </div>
    </section>
  </section>
</template>
