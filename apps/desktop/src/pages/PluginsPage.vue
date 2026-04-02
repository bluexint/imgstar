<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { KvReadonlySnapshot } from "@imgstar/contracts";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "@/stores/pluginStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";
import { api } from "@/services/api";
import {
  buildWafObjectExpression,
} from "@/utils/waf";

const { t } = useI18n();
const pluginStore = usePluginStore();
const settingsStore = useSettingsStore();
const toastStore = useToastStore();
const kvReadonlySnapshot = ref<KvReadonlySnapshot>({
  digitCount: 9,
  objects: []
});
let snapshotTimer: number | undefined;

const supportsConfig = (id: string): boolean =>
  id === "image-compress" || id === "hidden-watermark";

const refreshKvReadonlySnapshot = async (): Promise<void> => {
  try {
    kvReadonlySnapshot.value = await api.getKvReadonlySnapshot();
  } catch {
    kvReadonlySnapshot.value = {
      digitCount: Math.min(20, Math.max(1, settingsStore.persisted.digitCount ?? 9)),
      objects: []
    };
  }
};

onMounted(() => {
  void refreshKvReadonlySnapshot();
  snapshotTimer = window.setInterval(() => {
    void refreshKvReadonlySnapshot();
  }, 4000);
});

onUnmounted(() => {
  if (snapshotTimer !== undefined) {
    window.clearInterval(snapshotTimer);
  }
});

const wafDigitCount = computed(() => {
  const backendDigitCount = kvReadonlySnapshot.value.digitCount;
  if (Number.isFinite(backendDigitCount) && backendDigitCount > 0) {
    return Math.min(20, Math.max(1, backendDigitCount));
  }

  return Math.min(20, Math.max(1, settingsStore.persisted.digitCount ?? 9));
});

const wafObjects = computed(() => kvReadonlySnapshot.value.objects);
const wafObjectKeys = computed(() => wafObjects.value.map((entry) => entry.objectKey));
const wafObjectCount = computed(() => wafObjects.value.length);

const wafExpression = computed(
  () => buildWafObjectExpression(wafObjectKeys.value, settingsStore.persisted.cdnBaseUrl)
);

const hasWafConfig = computed(() => {
  const zoneId = settingsStore.persisted.zoneId?.trim() ?? "";
  const zoneApiToken = settingsStore.persisted.zoneApiToken?.trim() ?? "";
  const cdnBaseUrl = settingsStore.persisted.cdnBaseUrl?.trim() ?? "";
  return zoneId.length > 0 && zoneApiToken.length > 0 && cdnBaseUrl.length > 0;
});

const onNumberInput = (
  key: "maxEdge" | "quality" | "watermarkOpacity",
  event: Event
): void => {
  const target = event.target as HTMLInputElement;
  const value = Number(target.value);

  if (Number.isNaN(value)) {
    return;
  }

  if (key === "maxEdge") {
    pluginStore.setImageOption("maxEdge", Math.max(320, Math.round(value)));
    return;
  }

  if (key === "quality") {
    pluginStore.setImageOption("quality", Math.min(1, Math.max(0.4, value)));
    return;
  }

  pluginStore.setImageOption("watermarkOpacity", Math.min(0.8, Math.max(0.08, value)));
};

const onWatermarkInput = (event: Event): void => {
  const target = event.target as HTMLInputElement;
  pluginStore.setImageOption("watermarkText", target.value);
};

const onTogglePlugin = async (
  id: string,
  nextEnabled: boolean,
  nameKey: string
): Promise<void> => {
  const result = await pluginStore.setPluginEnabled(id, nextEnabled);
  if (!nextEnabled) {
    return;
  }

  const pluginName = String(t(nameKey));
  if (result.success && result.verified) {
    toastStore.pushSuccess(String(t("plugins.enabledToast", { name: pluginName })));
    return;
  }

  toastStore.pushWarn(
    String(t("plugins.signatureRejectedToast", { name: pluginName }))
  );
};
</script>

<template>
  <section class="flex h-full flex-col gap-4">
    <header class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">{{ t("plugins.title") }}</h2>
    </header>

    <div class="grid gap-3">
      <article
        v-for="plugin in pluginStore.plugins"
        :key="plugin.id"
        class="motion-panel rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]"
      >
        <div class="flex items-start justify-between gap-3">
          <div>
            <h3 class="font-semibold">{{ t(plugin.nameKey) }}</h3>
            <p class="text-sm text-[var(--text-muted)]">{{ t(plugin.descriptionKey) }}</p>
            <p v-if="plugin.error" class="mt-1 text-xs text-[var(--state-failed-text)]">{{ plugin.error }}</p>
          </div>
          <button
            class="motion-press rounded-xl bg-[var(--accent)] px-3 py-1.5 text-xs text-white hover:bg-[var(--accent-strong)]"
            type="button"
            @click="onTogglePlugin(plugin.id, plugin.status !== 'enabled', plugin.nameKey)"
          >
            {{ plugin.status === "enabled" ? t("plugins.disable") : t("plugins.enable") }}
          </button>
        </div>

        <details
          v-if="supportsConfig(plugin.id)"
          class="group mt-3 rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/45 motion-panel"
        >
          <summary class="flex cursor-pointer list-none items-center justify-between px-3 py-2 text-xs text-[var(--text-muted)]">
            <span>{{ t("plugins.config") }}</span>
            <span class="group-open:hidden">{{ t("plugins.expand") }}</span>
            <span class="hidden group-open:inline">{{ t("plugins.collapse") }}</span>
          </summary>

          <div class="border-t border-[var(--border-muted)] px-3 py-3">
            <p class="text-[11px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
              {{ plugin.hookType }} / {{ plugin.stage }} / priority {{ plugin.priority }}
            </p>

            <div v-if="plugin.id === 'image-compress'" class="mt-3 grid gap-3 md:grid-cols-2">
              <label class="grid gap-1 text-xs text-[var(--text-muted)]">
                <span>{{ t('plugins.maxEdgeLabel') }}</span>
                <input
                  :value="pluginStore.imageOptions.maxEdge"
                  class="motion-field rounded-md border border-[var(--border-muted)] bg-[var(--bg-main)] px-2 py-1.5 text-sm text-[var(--text-main)]"
                  max="16384"
                  min="320"
                  step="1"
                  type="number"
                  @input="onNumberInput('maxEdge', $event)"
                />
              </label>

              <label class="grid gap-1 text-xs text-[var(--text-muted)]">
                <span>{{ t('plugins.qualityLabel') }}: {{ pluginStore.imageOptions.quality.toFixed(2) }}</span>
                <input
                  :value="pluginStore.imageOptions.quality"
                  max="1"
                  min="0.4"
                  step="0.01"
                  type="range"
                  @input="onNumberInput('quality', $event)"
                />
              </label>
            </div>

            <div v-if="plugin.id === 'hidden-watermark'" class="mt-3 grid gap-3 md:grid-cols-2">
              <label class="grid gap-1 text-xs text-[var(--text-muted)]">
                <span>{{ t('plugins.watermarkTextLabel') }}</span>
                <input
                  :value="pluginStore.imageOptions.watermarkText"
                  class="motion-field rounded-md border border-[var(--border-muted)] bg-[var(--bg-main)] px-2 py-1.5 text-sm text-[var(--text-main)]"
                  maxlength="48"
                  type="text"
                  @input="onWatermarkInput"
                />
              </label>

              <label class="grid gap-1 text-xs text-[var(--text-muted)]">
                <span>{{ t('plugins.watermarkOpacityLabel') }}: {{ pluginStore.imageOptions.watermarkOpacity.toFixed(2) }}</span>
                <input
                  :value="pluginStore.imageOptions.watermarkOpacity"
                  max="0.8"
                  min="0.08"
                  step="0.01"
                  type="range"
                  @input="onNumberInput('watermarkOpacity', $event)"
                />
              </label>
            </div>
          </div>
        </details>
      </article>
    </div>

    <article class="motion-panel rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
      <div class="flex items-start justify-between gap-3">
        <div>
          <h3 class="font-semibold">{{ t('plugins.wafName') }}</h3>
          <p class="text-sm text-[var(--text-muted)]">{{ t('plugins.wafDescription') }}</p>
        </div>
        <span
          class="rounded-full px-2.5 py-1 text-[11px] font-medium uppercase tracking-[0.12em]"
          :class="hasWafConfig
            ? 'bg-[var(--state-success-bg)] text-[var(--state-success-text)]'
            : 'bg-[var(--bg-muted)] text-[var(--text-muted)]'"
        >
          {{ hasWafConfig ? t('plugins.wafReady') : t('plugins.wafPending') }}
        </span>
      </div>

      <div class="mt-4 grid gap-3 lg:grid-cols-2">
        <details
          class="group rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35 motion-panel"
        >
          <summary class="flex cursor-pointer list-none items-center justify-between px-3 py-2">
            <p class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
              {{ t('plugins.wafRulesTitle') }}
            </p>
            <span class="text-xs text-[var(--text-muted)] group-open:hidden">{{ t('plugins.expand') }}</span>
            <span class="hidden text-xs text-[var(--text-muted)] group-open:inline">{{ t('plugins.collapse') }}</span>
          </summary>

          <div class="border-t border-[var(--border-muted)] p-3">
            <p class="mt-2 text-xs text-[var(--text-muted)]">
              {{ t('plugins.wafDigitCount', { count: wafDigitCount }) }}
            </p>
            <p class="mt-3 text-[11px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
              {{ t('plugins.wafExpressionLabel') }}
            </p>
            <p class="mt-3 break-all rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 font-mono text-xs leading-6 text-[var(--text-main)]">
              {{ wafExpression }}
            </p>
            <p class="mt-2 text-[11px] text-[var(--text-muted)]">
              {{ t('plugins.wafObjectCount', { count: wafObjectCount }) }}
            </p>
          </div>
        </details>

        <details
          class="group rounded-xl border border-[var(--border-muted)] bg-[var(--bg-muted)]/35 motion-panel"
        >
          <summary class="flex cursor-pointer list-none items-center justify-between px-3 py-2">
            <p class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">
              {{ t('plugins.wafObservabilityTitle') }}
            </p>
            <span class="text-xs text-[var(--text-muted)] group-open:hidden">{{ t('plugins.expand') }}</span>
            <span class="hidden text-xs text-[var(--text-muted)] group-open:inline">{{ t('plugins.collapse') }}</span>
          </summary>

          <div class="border-t border-[var(--border-muted)] p-3">
            <div class="grid gap-2 text-sm text-[var(--text-main)]">
              <div class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2">
                {{ t('plugins.wafEventLabel') }}
              </div>
              <div class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2">
                {{ t('plugins.wafSourceLabel') }}
              </div>
              <div class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-xs leading-6 text-[var(--text-muted)]">
                {{ t('plugins.wafHint') }}
              </div>
              <div class="rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-xs leading-5 text-[var(--text-muted)]">
                <p class="text-[11px] uppercase tracking-[0.12em]">{{ t('plugins.wafObjectsTitle') }}</p>
                <p class="mt-1 text-[10px] uppercase tracking-[0.1em]">{{ t('plugins.wafObjectsReadonly') }}</p>
                <p v-if="wafObjects.length === 0" class="mt-2">
                  {{ t('plugins.wafObjectsEmpty') }}
                </p>
                <div v-else class="mt-2 grid gap-2">
                  <article
                    v-for="entry in wafObjects"
                    :key="entry.number"
                    class="rounded-md border border-[var(--border-muted)] bg-[var(--bg-muted)]/40 px-2 py-1.5"
                  >
                    <p class="font-mono text-[11px] text-[var(--text-main)]">
                      {{ entry.objectKey.startsWith('/') ? entry.objectKey : `/${entry.objectKey}` }}
                    </p>
                    <p class="mt-1 text-[10px] uppercase tracking-[0.1em]">
                      {{ t('plugins.wafNumberLabel') }}: {{ entry.number }}
                    </p>
                  </article>
                </div>
              </div>
            </div>
          </div>
        </details>
      </div>
    </article>
  </section>
</template>
