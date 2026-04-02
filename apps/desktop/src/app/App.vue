<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { RouterView } from "vue-router";
import { useI18n } from "vue-i18n";
import SidebarNav from "@/widgets/SidebarNav.vue";
import StatusBar from "@/widgets/StatusBar.vue";
import ToastHost from "@/widgets/ToastHost.vue";
import { getTheme, toggleTheme } from "@/theme/tokens";
import { setLocale, type Locale } from "@/i18n/setup";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUploadStore } from "@/stores/uploadStore";

const { t, locale } = useI18n();
const settingsStore = useSettingsStore();
const uploadStore = useUploadStore();
const router = useRouter();

const themeMode = ref(getTheme());
const mainRef = ref<HTMLElement | null>(null);
const scrollOffset = ref(0);
const showInitRiskDialog = ref(false);
const initRiskDismissed = ref(false);
let rafId = 0;

const localeModel = computed({
  get: () => locale.value as Locale,
  set: (value: Locale) => {
    setLocale(value);
  }
});

const themeLabel = computed(() =>
  themeMode.value === "light" ? t("theme.light") : t("theme.dark")
);

const onToggleTheme = (): void => {
  themeMode.value = toggleTheme();
};

watch(
  () => settingsStore.isConfigured,
  (configured) => {
    if (configured) {
      initRiskDismissed.value = false;
      showInitRiskDialog.value = false;
      return;
    }

    showInitRiskDialog.value = !initRiskDismissed.value;
  }
);

const dismissInitRiskDialog = (): void => {
  initRiskDismissed.value = true;
  showInitRiskDialog.value = false;
};

const openInitializationSettings = async (): Promise<void> => {
  initRiskDismissed.value = true;
  showInitRiskDialog.value = false;
  if (router.currentRoute.value.path !== "/settings") {
    await router.push("/settings");
  }
};

const updateScrollOffset = (): void => {
  const main = mainRef.value;
  if (!main) {
    return;
  }

  scrollOffset.value = main.scrollTop;
};

const onMainScroll = (): void => {
  if (rafId !== 0) {
    return;
  }

  rafId = window.requestAnimationFrame(() => {
    updateScrollOffset();
    rafId = 0;
  });
};

onMounted(async () => {
  await settingsStore.hydrate();
  await uploadStore.hydrate();
  showInitRiskDialog.value = !settingsStore.isConfigured && !initRiskDismissed.value;
  updateScrollOffset();
});

onUnmounted(() => {
  if (rafId !== 0) {
    window.cancelAnimationFrame(rafId);
  }
});
</script>

<template>
  <div class="h-screen min-h-[600px] min-w-[800px] bg-[var(--bg-main)] text-[var(--text-main)]">
    <div class="grid h-[calc(100%-32px)] grid-cols-[64px_1fr]">
      <SidebarNav />
      <div class="flex h-full flex-col overflow-hidden border-l border-[var(--border-muted)] bg-[var(--bg-panel)]">
        <header class="flex h-12 items-center justify-between border-b border-[var(--border-muted)] bg-[linear-gradient(135deg,rgba(148,163,184,0.12),transparent_55%)] px-4">
          <h1 class="text-sm font-semibold tracking-[0.08em]">{{ t("app.title") }}</h1>
          <div class="flex items-center gap-3 text-xs">
            <button
              class="motion-press rounded-xl bg-[var(--bg-muted)] px-3 py-1 text-[var(--text-main)] hover:bg-[var(--accent)] hover:text-white"
              type="button"
              @click="onToggleTheme"
            >
              {{ t("header.theme") }}: {{ themeLabel }}
            </button>
            <label class="flex items-center gap-2">
              <span>{{ t("header.language") }}</span>
              <select
                v-model="localeModel"
                class="motion-field rounded-xl border border-[var(--border-muted)] bg-[linear-gradient(135deg,rgba(148,163,184,0.12),transparent_60%)] px-2 py-1 outline-none focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]"
              >
                <option value="zh-CN">zh-CN</option>
                <option value="en">en</option>
              </select>
            </label>
          </div>
        </header>

        <main
          ref="mainRef"
          :style="{ '--scroll-offset': `${scrollOffset}px` }"
          class="relative h-[calc(100%-48px)] overflow-auto p-4"
          @scroll.passive="onMainScroll"
        >
          <div class="pointer-events-none absolute inset-x-0 top-0 h-56 overflow-hidden">
            <div
              class="motion-parallax-layer absolute right-[-3rem] top-[-1rem] h-40 w-40 rounded-full bg-[radial-gradient(circle,rgba(96,165,250,0.22),rgba(96,165,250,0)_68%)] blur-3xl"
              :style="{ transform: `translate3d(0, ${scrollOffset * 0.08}px, 0)` }"
            ></div>
            <div
              class="motion-parallax-layer absolute left-[-4rem] top-10 h-48 w-48 rounded-full bg-[radial-gradient(circle,rgba(59,130,246,0.14),rgba(59,130,246,0)_70%)] blur-3xl"
              :style="{ transform: `translate3d(0, ${scrollOffset * 0.04}px, 0)` }"
            ></div>
          </div>

          <RouterView v-slot="{ Component }">
            <transition mode="out-in" name="page-fade">
              <component :is="Component" />
            </transition>
          </RouterView>
        </main>
      </div>
    </div>

    <StatusBar />
    <ToastHost />

    <div
      v-if="showInitRiskDialog"
      class="fixed inset-0 z-[70] grid place-items-center bg-[var(--surface-overlay)] p-4 backdrop-blur-[2px]"
    >
      <section class="motion-panel w-full max-w-lg rounded-2xl border border-[var(--state-failed-border)] bg-[var(--bg-panel)] p-5 shadow-[0_45px_85px_-45px_rgba(15,23,42,0.82)]">
        <h2 class="text-base font-semibold text-[var(--state-failed-text)]">
          {{ t("settings.initRiskTitle") }}
        </h2>
        <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">
          {{ t("settings.initRiskDescription") }}
        </p>

        <div class="mt-4 flex items-center justify-end gap-2">
          <button
            class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-sm text-[var(--text-main)] hover:bg-[var(--bg-muted)]/70"
            type="button"
            @click="dismissInitRiskDialog"
          >
            {{ t("settings.initRiskLater") }}
          </button>
          <button
            class="motion-press rounded-lg bg-[var(--state-failed-text)] px-3 py-2 text-sm text-white hover:opacity-95"
            type="button"
            @click="openInitializationSettings"
          >
            {{ t("settings.initRiskGoSettings") }}
          </button>
        </div>
      </section>
    </div>
  </div>
</template>
