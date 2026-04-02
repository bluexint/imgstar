<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { UploadTaskStatus } from "@imgstar/contracts";
import { useLogStore } from "@/stores/logStore";
import { usePluginStore } from "@/stores/pluginStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUploadStore } from "@/stores/uploadStore";
import type { UploadTaskRuntimeState } from "@/stores/uploadStore";
import { formatDataRate, formatFileSize } from "@/utils/observability";

const { t } = useI18n();
const uploadStore = useUploadStore();
const pluginStore = usePluginStore();
const logStore = useLogStore();
const settingsStore = useSettingsStore();

const canStart = computed(() => uploadStore.hydrated && uploadStore.hasQueued && !uploadStore.uploading);
const canClear = computed(() => uploadStore.hydrated && !uploadStore.hasRunning);

const statusLabel = (status: UploadTaskStatus): string => {
  if (status === "draft") return "draft";
  if (status === "queued") return "queued";
  if (status === "running") return "running";
  if (status === "success") return "success";
  if (status === "failed") return "failed";
  return "cancelled";
};

const onPickFiles = async (event: Event): Promise<void> => {
  const input = event.target as HTMLInputElement;
  const fileList = input.files;
  if (!fileList || fileList.length === 0) {
    return;
  }

  await uploadStore.addPickedFiles(Array.from(fileList));
  input.value = "";
};

const onStart = async (): Promise<void> => {
  await uploadStore.startQueuedUploads({
    pluginChain: pluginStore.activeUploadChain,
    imageOptions: pluginStore.imageOptions
  });
  await Promise.all([logStore.refresh(true), settingsStore.refreshPing(true)]);
};

const onCancel = async (taskId: string): Promise<void> => {
  await uploadStore.cancelTask(taskId);
  await logStore.refresh(true);
};

const onRemove = async (taskId: string): Promise<void> => {
  await uploadStore.removeTask(taskId);
  await logStore.refresh(true);
};

const thumbnailOf = (taskId: string): string | undefined =>
  uploadStore.getThumbnail(taskId);

const formatSpeed = (task: UploadTaskRuntimeState): string =>
  formatDataRate(task.speedBps);

</script>

<template>
  <section class="flex h-full flex-col gap-4">
    <header>
      <h2 class="text-lg font-semibold">{{ t("upload.title") }}</h2>
      <p class="text-sm text-[var(--text-muted)]">{{ t("upload.subtitle") }}</p>
    </header>

    <div class="grid grid-cols-1 gap-3 rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)] motion-panel lg:grid-cols-[auto_auto_1fr]">
      <label
        class="motion-press inline-flex cursor-pointer items-center justify-center rounded-xl bg-[var(--accent)] px-3 py-2 text-sm text-white hover:bg-[var(--accent-strong)]"
      >
        {{ t("upload.pickFiles") }}
        <input class="hidden" multiple type="file" @change="onPickFiles" />
      </label>

      <label class="flex items-center gap-2 text-sm motion-field">
        <span>{{ t("upload.target") }}</span>
        <select
          :disabled="uploadStore.hasRunning || !uploadStore.hydrated"
          class="rounded-xl border border-[var(--border-muted)] bg-[linear-gradient(135deg,rgba(148,163,184,0.12),transparent_60%)] px-2 py-2 outline-none motion-field focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]"
          @change="uploadStore.setTarget(($event.target as HTMLSelectElement).value)"
        >
          <option
            v-for="target in uploadStore.targets"
            :key="target.id"
            :selected="target.id === uploadStore.target.id"
            :value="target.id"
          >
            {{ target.label }}
          </option>
        </select>
      </label>

      <div class="flex items-center justify-end gap-2">
        <button
          data-testid="start-upload"
          :disabled="!canStart"
          :aria-busy="uploadStore.uploading"
          class="motion-press rounded-xl bg-[var(--accent)] px-3 py-2 text-sm text-white hover:bg-[var(--accent-strong)] disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:translate-y-0"
          type="button"
          @click="onStart"
        >
          {{ t("upload.start") }}
        </button>
        <button
          :disabled="!canClear"
          class="motion-press rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-sm text-[var(--text-main)] hover:bg-[var(--bg-muted)]/60 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-[var(--bg-main)]"
          type="button"
          @click="uploadStore.clearList()"
        >
          {{ t("upload.clear") }}
        </button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-hidden rounded-xl border border-[var(--border-muted)] bg-[var(--bg-panel)] motion-panel">
      <div class="h-full overflow-auto">
        <table class="w-full border-collapse text-left text-sm">
          <thead class="sticky top-0 z-10 bg-[var(--bg-muted)] text-xs uppercase tracking-wide text-[var(--text-muted)]">
            <tr>
              <th class="px-3 py-2">{{ t("upload.file") }}</th>
              <th class="px-3 py-2">{{ t("upload.status") }}</th>
              <th class="px-3 py-2">{{ t("upload.progress") }}</th>
              <th class="px-3 py-2">{{ t("upload.traceId") }}</th>
              <th class="px-3 py-2">{{ t("upload.actions") }}</th>
            </tr>
          </thead>

          <tbody v-if="uploadStore.tasks.length === 0">
            <tr>
              <td class="px-3 py-8 text-center text-[var(--text-muted)]" colspan="5">
                {{ t("upload.empty") }}
              </td>
            </tr>
          </tbody>

          <TransitionGroup v-else appear name="upload-row" tag="tbody">
            <tr
              v-for="task in uploadStore.tasks"
              :key="task.id"
              class="group border-t border-[var(--border-muted)] hover:bg-[var(--bg-muted)]/35"
            >
              <td class="px-3 py-2">
                <div class="flex items-center gap-3">
                  <div class="h-12 w-12 overflow-hidden rounded-lg border border-[var(--border-muted)] bg-[var(--bg-muted)]/50">
                    <img
                      v-if="thumbnailOf(task.id)"
                      :src="thumbnailOf(task.id)"
                      :alt="task.file.name"
                      class="h-full w-full object-cover"
                    />
                    <div v-else class="grid h-full w-full place-items-center text-[10px] uppercase tracking-wider text-[var(--text-muted)]">
                      N/A
                    </div>
                  </div>
                  <div>
                    <div class="font-medium">{{ task.file.name }}</div>
                    <div class="text-xs text-[var(--text-muted)]">{{ formatFileSize(task.file.size) }}</div>
                    <div class="mt-1 text-[11px] uppercase tracking-[0.08em] text-[var(--text-placeholder)]">
                      {{ t("upload.speed") }}: {{ formatSpeed(task) }}
                    </div>
                  </div>
                </div>
              </td>
              <td class="px-3 py-2" data-testid="row-status">
                <span class="rounded bg-[var(--bg-muted)] px-2 py-1 text-xs font-medium">{{ statusLabel(task.status) }}</span>
              </td>
              <td class="px-3 py-2">
                <div class="h-2 w-full rounded-full bg-[var(--bg-muted)]">
                  <div
                    class="h-2 rounded-full bg-[var(--accent)] transition-all duration-200 ease-out"
                    :style="{ width: `${task.progress}%` }"
                  ></div>
                </div>
              </td>
              <td class="px-3 py-2 text-xs text-[var(--text-muted)]">{{ task.traceId ?? "-" }}</td>
              <td class="px-3 py-2">
                <div class="flex items-center gap-2">
                  <button
                    v-if="task.status === 'queued' || task.status === 'running'"
                    class="motion-press rounded-md border border-[var(--border-muted)] bg-[var(--bg-main)] px-2 py-1 text-xs text-[var(--text-main)] hover:bg-[var(--bg-muted)]/75"
                    type="button"
                    @click="onCancel(task.id)"
                  >
                    {{ t("upload.cancel") }}
                  </button>
                  <button
                    v-if="task.status === 'failed' || task.status === 'cancelled'"
                    class="motion-press rounded-md bg-[var(--accent)] px-2 py-1 text-xs text-white hover:bg-[var(--accent-strong)]"
                    type="button"
                    @click="uploadStore.retryTask(task.id)"
                  >
                    {{ t("upload.retry") }}
                  </button>
                  <button
                    v-if="task.status !== 'running'"
                    class="motion-press rounded-md border border-[var(--state-failed-border)] bg-[var(--state-failed-bg)] px-2 py-1 text-xs text-[var(--state-failed-text)] hover:bg-[var(--state-failed-bg)]/80"
                    type="button"
                    @click="onRemove(task.id)"
                  >
                    {{ t("upload.remove") }}
                  </button>
                </div>
              </td>
            </tr>
          </TransitionGroup>
        </table>
      </div>
    </div>
  </section>
</template>

<style scoped>
.upload-row-enter-active,
.upload-row-leave-active {
  transition:
    opacity 220ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1),
    background-color 220ms ease;
}

.upload-row-enter-from,
.upload-row-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.99);
}

.upload-row-move {
  transition: transform 220ms ease;
}
</style>
