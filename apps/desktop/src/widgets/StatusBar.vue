<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUploadStore } from "@/stores/uploadStore";
import { useSettingsStore } from "@/stores/settingsStore";

const { t } = useI18n();
const uploadStore = useUploadStore();
const settingsStore = useSettingsStore();

const statusText = computed(() => {
  if (!settingsStore.isConfigured) {
    return t("status.unconfigured");
  }

  return uploadStore.hasRunning ? t("status.busy") : t("status.connected");
});

const statusDotClass = computed(() => {
  if (!settingsStore.isConfigured) {
    return "bg-amber-500";
  }

  return uploadStore.hasRunning ? "bg-sky-500" : "bg-emerald-500";
});

const pingText = computed(() => {
  if (!settingsStore.isConfigured) {
    return "";
  }

  if (settingsStore.pingRefreshing) {
    return t("status.pingChecking");
  }

  if (settingsStore.pingMs === null) {
    return t("status.pingUnavailable");
  }

  return `${t("status.ping")}: ${settingsStore.pingMs}ms`;
});

const onRefreshPing = (): void => {
  if (!settingsStore.isConfigured) {
    return;
  }

  void settingsStore.refreshPing(true);
};

onMounted(() => {
  if (settingsStore.isConfigured && settingsStore.pingMs === null) {
    void settingsStore.refreshPing(true);
  }
});

watch(
  () => settingsStore.isConfigured,
  (configured, previousConfigured) => {
    if (configured && !previousConfigured) {
      void settingsStore.refreshPing(true);
    }
  }
);

const counters = computed(() => uploadStore.counters);

const counterEntries = computed(() => [
  { key: "running", label: "running", value: counters.value.running },
  { key: "success", label: "success", value: counters.value.success },
  { key: "failed", label: "failed", value: counters.value.failed }
]);
</script>

<template>
  <footer
    class="flex h-8 items-center justify-between border-t border-[var(--border-muted)] bg-[linear-gradient(135deg,rgba(148,163,184,0.1),transparent_55%)] px-4 text-xs"
  >
    <button
      :class="settingsStore.isConfigured ? 'hover:bg-[var(--bg-muted)]/55' : ''"
      :disabled="!settingsStore.isConfigured"
      :title="settingsStore.lastPingAt"
      class="flex items-center gap-2 rounded-md border-0 bg-transparent px-1.5 py-0.5 text-left"
      type="button"
      @click="onRefreshPing"
    >
      <span :class="statusDotClass" class="status-dot h-2 w-2 rounded-full"></span>
      <span>{{ statusText }}</span>
      <span
        v-if="settingsStore.isConfigured"
        class="rounded-md bg-[var(--bg-muted)]/60 px-1.5 py-0.5 text-[10px] tracking-[0.06em] text-[var(--text-muted)]"
      >
        {{ pingText }}
      </span>
    </button>

    <div class="flex items-center gap-2 text-[var(--text-muted)]">
      <div
        v-for="item in counterEntries"
        :key="item.key"
        class="flex items-center gap-1 rounded-md bg-[var(--bg-muted)]/55 px-1.5 py-0.5"
      >
        <span class="text-[10px] uppercase tracking-[0.08em]">{{ item.label }}</span>
        <Transition mode="out-in" name="counter-pop">
          <span :key="`${item.key}-${item.value}`" class="font-semibold text-[var(--text-main)]">
            {{ item.value }}
          </span>
        </Transition>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.status-dot {
  box-shadow: 0 0 0 3px rgba(148, 163, 184, 0.25);
}

.counter-pop-enter-active,
.counter-pop-leave-active {
  transition: all 180ms ease;
}

.counter-pop-enter-from {
  opacity: 0;
  transform: translateY(4px) scale(0.92);
}

.counter-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(1.08);
}
</style>
