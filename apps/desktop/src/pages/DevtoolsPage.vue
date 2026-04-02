<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import type { UploadEvent } from "@imgstar/contracts";
import { useLogStore } from "@/stores/logStore";
import {
  buildKvBucketSummaries,
  buildKvSnapshot,
  buildKvTrendPoints,
  createStateCounter,
  formatKvTimestamp,
  stateOrder,
  type KvEntryState
} from "@/utils/observability";

type DevtoolsViewMode = "logs" | "kv";

const { t } = useI18n();
const route = useRoute();
const logStore = useLogStore();

const viewMode = ref<DevtoolsViewMode>("logs");
const moduleFilter = ref<UploadEvent["module"] | "">("");
const levelFilter = ref<UploadEvent["level"] | "">("");
const traceFilter = ref("");
const startAt = ref("");
const endAt = ref("");
const showFilters = ref(false);
const advancedMode = ref(false);
const kvKeyword = ref("");
const kvStateFilter = ref<KvEntryState | "all">("all");
const pinnedLevels = ref<UploadEvent["level"][] | null>(null);

let timer: number | undefined;

const DEFAULT_LEVELS: UploadEvent["level"][] = ["INFO", "WARN", "ERROR", "DEBUG"];

const controlButtonClass =
  "motion-press rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] px-2.5 py-2 text-xs text-[var(--text-main)] hover:bg-[var(--bg-muted)]/60";
const selectClass =
  "motion-field rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] px-2 py-2 text-sm text-[var(--text-main)] outline-none focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]";
const inputClass =
  "motion-field rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] px-2 py-2 text-sm text-[var(--text-main)] outline-none placeholder:text-[var(--text-placeholder)] placeholder:opacity-100 focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]";

const parseLevels = (raw: string): UploadEvent["level"][] =>
  raw
    .split(",")
    .map((item) => item.trim().toUpperCase())
    .filter(
      (item): item is UploadEvent["level"] =>
        ["INFO", "WARN", "ERROR", "DEBUG"].includes(item)
    );

const rows = computed(() => logStore.events);
const kvSnapshot = computed(() => buildKvSnapshot(rows.value));

const kvVisibleEntries = computed(() => {
  const keyword = kvKeyword.value.trim().toLowerCase();

  return kvSnapshot.value.filter((entry) => {
    if (kvStateFilter.value !== "all" && entry.state !== kvStateFilter.value) {
      return false;
    }

    if (!keyword) {
      return true;
    }

    return [entry.objectKey, entry.number, entry.traceId, entry.fileName, entry.bucketLabel].some(
      (value) => value?.toLowerCase().includes(keyword)
    );
  });
});

const kvStateCounter = computed(() => {
  const counter = createStateCounter();

  for (const entry of kvVisibleEntries.value) {
    counter[entry.state] += 1;
  }

  return counter;
});

const latestNumber = computed(() => {
  const values = kvVisibleEntries.value
    .map((entry) => {
      const digits = (entry.number ?? "").replace(/\D/g, "");
      if (!digits) {
        return null;
      }

      const parsed = Number(digits);
      return Number.isFinite(parsed) ? parsed : null;
    })
    .filter((value): value is number => value !== null);

  if (values.length === 0) {
    return "-";
  }

  return String(Math.max(...values)).padStart(9, "0");
});

const allocatedCount = computed(() =>
  kvVisibleEntries.value.filter((entry) => entry.lastEvent === "upload:key_allocated").length
);

const kvSummaryCards = computed(() => [
  {
    key: "total",
    label: t("devtools.kv.cardTotalKeys"),
    value: String(kvVisibleEntries.value.length)
  },
  {
    key: "allocated",
    label: t("devtools.kv.cardAllocated"),
    value: String(allocatedCount.value)
  },
  {
    key: "latest",
    label: t("devtools.kv.cardLatestNumber"),
    value: latestNumber.value
  },
  {
    key: "active",
    label: t("devtools.kv.cardActive"),
    value: String(kvStateCounter.value.active)
  },
  {
    key: "failed",
    label: t("devtools.kv.cardFailed"),
    value: String(kvStateCounter.value.failed)
  }
]);

const kvBuckets = computed(() => buildKvBucketSummaries(kvVisibleEntries.value));
const kvTrendPoints = computed(() => buildKvTrendPoints(kvVisibleEntries.value));

const kvStateOptions = computed(() => [
  { value: "all" as const, label: t("devtools.kv.stateAll") },
  ...stateOrder.map((state) => ({ value: state, label: kvStateLabel(state) }))
]);

const activeFilterCount = computed(() => {
  let count = 0;
  if (moduleFilter.value) count += 1;
  if (levelFilter.value || (pinnedLevels.value && pinnedLevels.value.length > 0)) count += 1;
  if (traceFilter.value) count += 1;
  if (startAt.value || endAt.value) count += 1;
  return count;
});

const modeCountText = computed(() =>
  viewMode.value === "logs"
    ? `${rows.value.length} logs`
    : `${kvVisibleEntries.value.length} kv keys`
);

const levelClass = (level: UploadEvent["level"]): string => {
  if (level === "ERROR") {
    return "bg-[var(--toast-error-bg)] text-[var(--toast-error-text)]";
  }
  if (level === "WARN") {
    return "bg-[var(--toast-warn-bg)] text-[var(--toast-warn-text)]";
  }
  if (level === "DEBUG") {
    return "bg-[var(--bg-muted)] text-[var(--text-muted)]";
  }
  return "bg-[var(--toast-info-bg)] text-[var(--toast-info-text)]";
};

const kvStateClass = (state: KvEntryState): string => {
  if (state === "active") {
    return "bg-[var(--state-active-bg)] text-[var(--state-active-text)]";
  }
  if (state === "recycling") {
    return "bg-[var(--state-recycling-bg)] text-[var(--state-recycling-text)]";
  }
  if (state === "recycled") {
    return "bg-[var(--state-recycled-bg)] text-[var(--state-recycled-text)]";
  }
  if (state === "failed") {
    return "bg-[var(--state-failed-bg)] text-[var(--state-failed-text)]";
  }
  return "bg-[var(--state-reserved-bg)] text-[var(--state-reserved-text)]";
};

const kvStateBarClass = (state: KvEntryState): string => {
  if (state === "active") {
    return "bg-[var(--state-active-text)]";
  }
  if (state === "recycling") {
    return "bg-[var(--state-recycling-text)]";
  }
  if (state === "recycled") {
    return "bg-[var(--state-recycled-text)]";
  }
  if (state === "failed") {
    return "bg-[var(--state-failed-text)]";
  }
  return "bg-[var(--state-reserved-text)]";
};

const kvStateLabel = (state: KvEntryState): string => {
  if (state === "active") {
    return t("devtools.kv.stateActive");
  }
  if (state === "recycling") {
    return t("devtools.kv.stateRecycling");
  }
  if (state === "recycled") {
    return t("devtools.kv.stateRecycled");
  }
  if (state === "failed") {
    return t("devtools.kv.stateFailed");
  }
  return t("devtools.kv.stateReserved");
};

const toggleView = (mode: DevtoolsViewMode): void => {
  viewMode.value = mode;
};

const toggleLive = (): void => {
  logStore.toggleLive();
};

const toggleFilters = (): void => {
  showFilters.value = !showFilters.value;
};

const toggleAdvanced = (): void => {
  advancedMode.value = !advancedMode.value;
};

const clearFilters = async (): Promise<void> => {
  moduleFilter.value = "";
  levelFilter.value = "";
  traceFilter.value = "";
  startAt.value = "";
  endAt.value = "";
  pinnedLevels.value = null;
  await applyFilters();
};

const clearKvFilters = (): void => {
  kvKeyword.value = "";
  kvStateFilter.value = "all";
};

const applyFilters = async (): Promise<void> => {
  logStore.setModule(moduleFilter.value || undefined);
  logStore.filter.traceId = traceFilter.value || undefined;

  if (levelFilter.value) {
    pinnedLevels.value = null;
    logStore.setLevel(levelFilter.value);
  } else if (pinnedLevels.value && pinnedLevels.value.length > 0) {
    logStore.setLevels(pinnedLevels.value);
  } else {
    logStore.setLevels(DEFAULT_LEVELS);
  }

  const startISO = startAt.value ? new Date(startAt.value).toISOString() : undefined;
  const endISO = endAt.value ? new Date(endAt.value).toISOString() : undefined;
  logStore.setTimeRange(startISO, endISO);

  await logStore.refresh(true);
};

const formatContext = (context: Record<string, unknown>): string =>
  JSON.stringify(context, null, 2);

const getEventStack = (event: UploadEvent): string => {
  if (typeof event.stack === "string" && event.stack.trim().length > 0) {
    return event.stack;
  }

  const failureStack = event.context.failureStack;
  if (typeof failureStack === "string" && failureStack.trim().length > 0) {
    return failureStack;
  }

  const contextStack = event.context.stack;
  if (typeof contextStack === "string" && contextStack.trim().length > 0) {
    return contextStack;
  }

  return "";
};

const exportCurrentView = (): void => {
  const payload = viewMode.value === "logs" ? logStore.events : kvVisibleEntries.value;
  const suffix = viewMode.value === "logs" ? "logs" : "kv-state";

  const blob = new Blob([JSON.stringify(payload, null, 2)], {
    type: "application/json"
  });
  const url = window.URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `imgstar-${suffix}-${Date.now()}.json`;
  link.click();
  window.URL.revokeObjectURL(url);
};

const clearLogs = async (): Promise<void> => {
  if (viewMode.value !== "logs") {
    return;
  }

  try {
    await logStore.clear();
  } catch {
    logStore.clearLocal();
  }
};

watch(
  () => route.query.traceId,
  async (traceId) => {
    if (typeof traceId !== "string" || !traceId) {
      pinnedLevels.value = null;
      return;
    }

    viewMode.value = "logs";
    traceFilter.value = traceId;
    logStore.setTraceFocus(traceId);

    if (typeof route.query.levels === "string") {
      const levels = parseLevels(route.query.levels);
      pinnedLevels.value = levels;
      logStore.setLevels(levels);
    }

    await applyFilters();
  },
  { immediate: true }
);

onMounted(async () => {
  await logStore.refresh(true);
  timer = window.setInterval(() => {
    void logStore.refresh();
  }, 1000);
});

onUnmounted(() => {
  if (timer !== undefined) {
    window.clearInterval(timer);
  }
});
</script>

<template>
  <section class="flex h-full min-h-0 flex-col gap-3">
    <header class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">{{ t("devtools.title") }}</h2>
      <p class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
        {{ modeCountText }}
      </p>
    </header>

    <div class="motion-panel rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] shadow-[0_16px_35px_-30px_rgba(15,23,42,0.65)]">
      <div class="flex flex-wrap items-center gap-2 p-3">
        <button
          :class="[
            'rounded-xl border px-2.5 py-2 text-xs transition duration-200',
            viewMode === 'logs'
              ? 'border-transparent bg-[var(--accent)] text-slate-900'
              : controlButtonClass
          ]"
          type="button"
          @click="toggleView('logs')"
        >
          {{ t("devtools.viewLogs") }}
        </button>

        <button
          :class="[
            'rounded-xl border px-2.5 py-2 text-xs transition duration-200',
            viewMode === 'kv'
              ? 'border-transparent bg-[var(--accent)] text-slate-900'
              : controlButtonClass
          ]"
          type="button"
          @click="toggleView('kv')"
        >
          {{ t("devtools.viewKv") }}
        </button>

        <span class="mx-1 h-5 w-px bg-[var(--border-muted)]"></span>

        <button
          class="motion-press rounded-xl bg-[var(--accent)] px-2.5 py-2 text-xs text-slate-900 hover:bg-[var(--accent-strong)]"
          type="button"
          @click="applyFilters"
        >
          {{ t("devtools.refresh") }}
        </button>

        <button
          v-if="viewMode === 'logs'"
          :class="controlButtonClass"
          type="button"
          @click="toggleLive"
        >
          {{ logStore.live ? t("devtools.pause") : t("devtools.resume") }}
        </button>

        <button :class="controlButtonClass" type="button" @click="exportCurrentView">
          {{ t("devtools.export") }}
        </button>

        <button v-if="viewMode === 'logs'" :class="controlButtonClass" type="button" @click="clearLogs">
          {{ t("devtools.clear") }}
        </button>

        <button
          v-if="viewMode === 'logs'"
          :class="controlButtonClass"
          type="button"
          @click="toggleFilters"
        >
          {{ t("devtools.filter") }} {{ activeFilterCount > 0 ? `(${activeFilterCount})` : "" }}
        </button>

        <button
          v-if="viewMode === 'logs'"
          :class="[
            'rounded-xl border px-2.5 py-2 text-xs transition duration-200',
            advancedMode
              ? 'border-transparent bg-[var(--accent)] text-slate-900'
              : controlButtonClass
          ]"
          type="button"
          @click="toggleAdvanced"
        >
          {{ t("devtools.advanced") }}: {{ advancedMode ? "ON" : "OFF" }}
        </button>
      </div>

      <div
        v-if="viewMode === 'logs' && showFilters"
        class="grid grid-cols-1 gap-2 border-t border-[var(--border-muted)] bg-[var(--bg-muted)]/25 p-3 lg:grid-cols-[repeat(5,minmax(0,1fr))_auto]"
      >
        <select v-model="moduleFilter" :class="selectClass">
          <option value="">module: all</option>
          <option value="upload">upload</option>
          <option value="plugin">plugin</option>
          <option value="storage">storage</option>
        </select>

        <select v-model="levelFilter" :class="selectClass">
          <option value="">level: all</option>
          <option value="INFO">INFO</option>
          <option value="WARN">WARN</option>
          <option value="ERROR">ERROR</option>
          <option value="DEBUG">DEBUG</option>
        </select>

        <input
          v-model="traceFilter"
          :class="inputClass"
          placeholder="trace id"
          type="text"
        />

        <input v-model="startAt" :class="selectClass" type="datetime-local" />

        <input v-model="endAt" :class="selectClass" type="datetime-local" />

        <button :class="controlButtonClass" type="button" @click="clearFilters">
          {{ t("devtools.reset") }}
        </button>
      </div>

      <div
        v-if="viewMode === 'kv'"
        class="grid grid-cols-1 gap-2 border-t border-[var(--border-muted)] bg-[var(--bg-muted)]/25 p-3 lg:grid-cols-[1fr_220px_auto]"
      >
        <input
          v-model="kvKeyword"
          :class="inputClass"
          :placeholder="t('devtools.kv.searchPlaceholder')"
          type="text"
        />

        <select v-model="kvStateFilter" :class="selectClass">
          <option
            v-for="option in kvStateOptions"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </option>
        </select>

        <button :class="controlButtonClass" type="button" @click="clearKvFilters">
          {{ t("devtools.reset") }}
        </button>
      </div>
    </div>

    <div
      v-if="viewMode === 'logs'"
      class="min-h-0 flex-1 overflow-hidden rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]"
    >
      <div class="h-full overflow-auto">
        <table class="min-w-[980px] w-full border-collapse text-left text-xs">
          <thead class="sticky top-0 z-10 bg-[var(--bg-muted)] uppercase text-[var(--text-muted)]">
            <tr>
              <th class="px-2 py-2">time</th>
              <th class="px-2 py-2">module</th>
              <th class="px-2 py-2">level</th>
              <th class="px-2 py-2">event</th>
              <th class="px-2 py-2">status</th>
              <th class="px-2 py-2">traceId</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="rows.length === 0">
              <td class="px-2 py-6 text-center text-[var(--text-muted)]" colspan="6">
                {{ t("devtools.noLogs") }}
              </td>
            </tr>

            <tr
              v-for="event in rows"
              :key="`${event.traceId}-${event.timestamp}-${event.eventName}`"
              class="border-t border-[var(--border-muted)] align-top"
            >
              <td class="whitespace-nowrap px-2 py-2 text-[11px]">{{ event.timestamp }}</td>
              <td class="px-2 py-2">
                <span class="rounded bg-[var(--bg-muted)] px-2 py-0.5 text-[11px]">{{ event.module }}</span>
              </td>
              <td class="px-2 py-2">
                <span :class="levelClass(event.level)" class="rounded px-2 py-0.5 text-[11px] font-semibold">
                  {{ event.level }}
                </span>
              </td>
              <td class="px-2 py-2">
                <div class="font-semibold">{{ event.eventName }}</div>

                <p v-if="advancedMode && (event.errorCode || event.errorMessage)" class="mt-1 break-all text-[11px] leading-5 text-[var(--state-failed-text)]">
                  {{ event.errorCode ?? "UNKNOWN_ERROR" }} {{ event.errorMessage ?? "" }}
                </p>

                <p v-if="advancedMode && getEventStack(event)" class="mt-1 break-all rounded-md bg-[var(--state-failed-bg)] px-2 py-1 font-mono text-[10px] text-[var(--state-failed-text)]">
                  stack: {{ getEventStack(event) }}
                </p>

                <details v-if="advancedMode" class="mt-1">
                  <summary class="cursor-pointer select-none text-[10px] uppercase tracking-[0.1em] text-[var(--text-muted)]">
                    context
                  </summary>
                  <pre class="mt-1 max-h-28 overflow-auto whitespace-pre-wrap break-all rounded-md bg-[var(--bg-muted)]/65 p-2 font-mono text-[10px]">{{ formatContext(event.context) }}</pre>
                </details>
              </td>
              <td class="px-2 py-2 text-[11px]">{{ event.status }}</td>
              <td class="break-all px-2 py-2 font-mono text-[11px]">{{ event.traceId }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div
      v-else
      class="min-h-0 flex-1 overflow-auto rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]"
    >
      <div class="grid gap-3 p-3 xl:grid-cols-[1.8fr_1fr]">
        <div class="grid gap-3">
          <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-5">
            <article
              v-for="card in kvSummaryCards"
              :key="card.key"
              class="motion-panel rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] p-3"
            >
              <p class="text-[11px] uppercase tracking-[0.1em] text-[var(--text-muted)]">{{ card.label }}</p>
              <p class="mt-1 text-base font-semibold">{{ card.value }}</p>
            </article>
          </div>

          <section class="motion-panel rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] p-3">
            <div class="flex items-center justify-between gap-2">
              <div>
                <p class="text-[11px] uppercase tracking-[0.1em] text-[var(--text-muted)]">
                  {{ t("devtools.kv.bucketTitle") }}
                </p>
                <p class="text-xs text-[var(--text-muted)]">
                  {{ t("devtools.kv.bucketSubtitle", { size: "100000" }) }}
                </p>
                <p class="text-[10px] uppercase tracking-[0.08em] text-[var(--text-placeholder)]">
                  {{ t("devtools.kv.readonlyHint") }}
                </p>
              </div>
              <span class="rounded-full bg-[var(--bg-muted)] px-2 py-0.5 text-[11px] text-[var(--text-muted)]">
                {{ kvBuckets.length }}
              </span>
            </div>

            <div v-if="kvBuckets.length > 0" class="mt-3 grid max-h-[320px] gap-2 overflow-auto pr-1">
              <article
                v-for="bucket in kvBuckets"
                :key="bucket.bucketIndex"
                class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-panel)] p-3"
              >
                <div class="flex items-center justify-between gap-3">
                  <div>
                    <p class="font-mono text-[11px]">{{ bucket.label }}</p>
                    <p class="text-[10px] uppercase tracking-[0.1em] text-[var(--text-muted)]">
                      {{ formatKvTimestamp(bucket.latestUpdatedAt) }}
                    </p>
                  </div>
                  <span class="rounded-full bg-[var(--bg-muted)] px-2 py-0.5 text-[11px] font-semibold">
                    {{ bucket.count }}
                  </span>
                </div>

                <div class="mt-2 h-2 overflow-hidden rounded-full bg-[var(--bg-muted)]">
                  <div
                    class="h-full rounded-full bg-[var(--accent)] transition-all duration-200"
                    :style="{ width: `${Math.max(2, (bucket.count / Math.max(1, kvVisibleEntries.length)) * 100)}%` }"
                  ></div>
                </div>

                <div class="mt-2 flex flex-wrap gap-1 text-[10px]">
                  <template v-for="state in stateOrder" :key="state">
                    <span
                      v-if="bucket.stateCounts[state] > 0"
                      :class="kvStateClass(state)"
                      class="rounded-full px-2 py-0.5 font-medium"
                    >
                      {{ kvStateLabel(state) }}: {{ bucket.stateCounts[state] }}
                    </span>
                  </template>
                </div>
              </article>
            </div>

            <p v-else class="mt-3 text-sm text-[var(--text-muted)]">
              {{ t("devtools.kv.noBucketData") }}
            </p>
          </section>
        </div>

        <section class="motion-panel rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] p-3">
          <div class="flex items-center justify-between gap-2">
            <div>
              <p class="text-[11px] uppercase tracking-[0.1em] text-[var(--text-muted)]">
                {{ t("devtools.kv.trendTitle") }}
              </p>
              <p class="text-xs text-[var(--text-muted)]">
                {{ t("devtools.kv.trendSubtitle", { slices: String(kvTrendPoints.length) }) }}
              </p>
            </div>
            <span class="rounded-full bg-[var(--bg-muted)] px-2 py-0.5 text-[11px] text-[var(--text-muted)]">
              {{ kvTrendPoints.length }}
            </span>
          </div>

          <div v-if="kvTrendPoints.length > 0" class="mt-3 space-y-2">
            <article
              v-for="point in kvTrendPoints"
              :key="point.key"
              class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-panel)] p-3"
            >
              <div class="flex items-center justify-between gap-2 text-xs text-[var(--text-muted)]">
                <span>{{ point.label }}</span>
                <span>{{ point.count }}</span>
              </div>

              <div class="mt-2 flex h-2 overflow-hidden rounded-full bg-[var(--bg-muted)]">
                <span
                  v-for="state in stateOrder"
                  :key="state"
                  :class="kvStateBarClass(state)"
                  :style="{ width: `${point.count === 0 ? 0 : (point.stateCounts[state] / point.count) * 100}%` }"
                ></span>
              </div>

              <div class="mt-2 flex flex-wrap gap-1 text-[10px]">
                <template v-for="state in stateOrder" :key="state">
                  <span
                    v-if="point.stateCounts[state] > 0"
                    :class="kvStateClass(state)"
                    class="rounded-full px-2 py-0.5 font-medium"
                  >
                    {{ kvStateLabel(state) }}: {{ point.stateCounts[state] }}
                  </span>
                </template>
              </div>
            </article>
          </div>

          <p v-else class="mt-3 text-sm text-[var(--text-muted)]">
            {{ t("devtools.kv.noTrendData") }}
          </p>
        </section>
      </div>

      <div class="px-3 pb-3">
        <div class="overflow-hidden rounded-xl border border-[var(--border-muted)]">
          <table class="min-w-[1080px] w-full border-collapse text-left text-xs">
            <thead class="bg-[var(--bg-muted)] uppercase text-[var(--text-muted)]">
              <tr>
                <th class="px-2 py-2">{{ t("devtools.kv.columnObjectKey") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnNumber") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnState") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnTraceId") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnUpdatedAt") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnEvent") }}</th>
                <th class="px-2 py-2">{{ t("devtools.kv.columnFile") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-if="kvVisibleEntries.length === 0">
                <td class="px-2 py-6 text-center text-[var(--text-muted)]" colspan="7">
                  {{ t("devtools.kv.empty") }}
                </td>
              </tr>

              <tr
                v-for="entry in kvVisibleEntries"
                :key="entry.keyId"
                class="border-t border-[var(--border-muted)]"
              >
                <td class="break-all px-2 py-2 font-mono text-[11px]">
                  {{ entry.objectKey ?? "-" }}
                </td>
                <td class="px-2 py-2 font-mono text-[11px]">{{ entry.number ?? "-" }}</td>
                <td class="px-2 py-2">
                  <span :class="kvStateClass(entry.state)" class="rounded px-2 py-0.5 text-[11px] font-semibold">
                    {{ kvStateLabel(entry.state) }}
                  </span>
                </td>
                <td class="break-all px-2 py-2 font-mono text-[11px]">{{ entry.traceId }}</td>
                <td class="px-2 py-2 text-[11px]">{{ formatKvTimestamp(entry.updatedAt) }}</td>
                <td class="px-2 py-2 text-[11px]">{{ entry.lastEvent }}</td>
                <td class="break-all px-2 py-2 text-[11px]">{{ entry.fileName ?? "-" }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </section>
</template>