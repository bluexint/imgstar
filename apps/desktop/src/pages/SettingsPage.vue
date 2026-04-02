<script setup lang="ts">
import { computed, ref } from "vue";
import { onBeforeRouteLeave, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";
import { useUploadStore } from "@/stores/uploadStore";

const { t } = useI18n();
const router = useRouter();
const settingsStore = useSettingsStore();
const toastStore = useToastStore();
const uploadStore = useUploadStore();

const showUnsavedDialog = ref(false);
const showResetAppDialog = ref(false);
const pendingRoute = ref<string | null>(null);
const allowBypassOnce = ref(false);

const fieldClass =
  "motion-field rounded-xl border border-[var(--border-muted)] bg-[linear-gradient(135deg,rgba(148,163,184,0.12),transparent_60%)] px-3 py-2.5 text-sm text-[var(--text-main)] shadow-[inset_0_1px_0_rgba(255,255,255,0.18)] outline-none placeholder:uppercase placeholder:tracking-[0.14em] placeholder:text-[var(--text-placeholder)] placeholder:opacity-100 focus:border-[var(--accent)] focus:bg-[var(--bg-panel)] focus:shadow-[0_0_0_3px_rgba(59,130,246,0.15)]";

const statusText = computed(() => settingsStore.status);
const canResetApp = computed(() => !uploadStore.hasRunning);
type EditableSettingsField =
  | "accessKey"
  | "secretKey"
  | "endpoint"
  | "bucket"
  | "zoneId"
  | "zoneApiToken"
  | "cdnBaseUrl";

const onInput = (field: EditableSettingsField, event: Event): void => {
  settingsStore.updateField(field, (event.target as HTMLInputElement).value);
};

const closeUnsavedDialog = (): void => {
  showUnsavedDialog.value = false;
  pendingRoute.value = null;
};

const closeResetAppDialog = (): void => {
  showResetAppDialog.value = false;
};

const confirmLeave = async (): Promise<void> => {
  const target = pendingRoute.value;
  closeUnsavedDialog();
  if (!target) {
    return;
  }

  allowBypassOnce.value = true;
  await router.push(target);
};

const confirmResetApp = async (): Promise<void> => {
  if (!canResetApp.value) {
    return;
  }

  await settingsStore.resetApp();

  if (settingsStore.status === "error") {
    toastStore.pushError(String(t("settings.resetAppFailed")));
    return;
  }

  closeResetAppDialog();
  window.location.reload();
};

onBeforeRouteLeave((to) => {
  if (allowBypassOnce.value) {
    allowBypassOnce.value = false;
    return true;
  }

  if (!settingsStore.isDirty) {
    return true;
  }

  pendingRoute.value = to.fullPath;
  showUnsavedDialog.value = true;
  return false;
});
</script>

<template>
  <section class="mx-auto flex max-w-3xl flex-col gap-4">
    <header>
      <h2 class="text-lg font-semibold">{{ t("settings.title") }}</h2>
    </header>

    <div class="grid gap-3 rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-4 shadow-[0_20px_45px_-35px_rgba(15,23,42,0.5)]">
      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Access Key</span>
        <input
          :value="settingsStore.draft.accessKey"
          :class="fieldClass"
          placeholder="access key"
          type="text"
          @input="onInput('accessKey', $event)"
        />
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Secret Key</span>
        <input
          :value="settingsStore.draft.secretKey"
          :class="fieldClass"
          placeholder="secret key"
          type="password"
          @input="onInput('secretKey', $event)"
        />
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Endpoint</span>
        <input
          :value="settingsStore.draft.endpoint"
          :class="fieldClass"
          placeholder="https://"
          type="text"
          @input="onInput('endpoint', $event)"
        />
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Bucket</span>
        <input
          :value="settingsStore.draft.bucket"
          :class="fieldClass"
          placeholder="bucket name"
          type="text"
          @input="onInput('bucket', $event)"
        />
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Zone ID</span>
        <input
          :value="settingsStore.draft.zoneId ?? ''"
          :class="fieldClass"
          placeholder="32-char Cloudflare zone ID"
          type="text"
          @input="onInput('zoneId', $event)"
        />
        <span class="text-xs text-[var(--text-muted)]">Required for zone-level WAF sync and CDN cache purge.</span>
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">Cloudflare API Token</span>
        <input
          :value="settingsStore.draft.zoneApiToken ?? ''"
          :class="fieldClass"
          placeholder="token with Zone WAF Write / cache purge permissions"
          type="password"
          @input="onInput('zoneApiToken', $event)"
        />
        <span class="text-xs text-[var(--text-muted)]">Used for zone-level WAF sync and CDN cache purge.</span>
      </label>

      <label class="grid gap-1 text-sm">
        <span class="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">CDN Base URL</span>
        <input
          :value="settingsStore.draft.cdnBaseUrl ?? ''"
          :class="fieldClass"
          placeholder="https://cdn.example.com"
          type="text"
          @input="onInput('cdnBaseUrl', $event)"
        />
      </label>

      <div class="flex items-center gap-2">
        <button
          class="motion-press rounded-xl bg-[var(--accent)] px-3 py-2 text-sm text-white hover:bg-[var(--accent-strong)]"
          type="button"
          @click="settingsStore.save()"
        >
          {{ t("settings.save") }}
        </button>
        <button
          class="motion-press rounded-xl border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-sm text-[var(--text-main)] hover:bg-[var(--bg-muted)]/60"
          type="button"
          @click="settingsStore.reset()"
        >
          {{ t("settings.reset") }}
        </button>
        <button
          class="motion-press rounded-xl border border-[var(--state-failed-border)] bg-[var(--state-failed-bg)] px-3 py-2 text-sm text-[var(--state-failed-text)] hover:bg-[var(--state-failed-bg)]/80"
          :disabled="!canResetApp"
          type="button"
          @click="showResetAppDialog = true"
        >
          {{ t("settings.resetApp") }}
        </button>
      </div>

      <p class="text-xs text-[var(--text-muted)]">status: {{ statusText }}</p>
      <p v-if="settingsStore.lastSavedAt" class="text-xs text-[var(--text-muted)]">
        {{ t("settings.savedAt") }}: {{ settingsStore.lastSavedAt }}
      </p>
    </div>

    <div
      v-if="showResetAppDialog"
      class="fixed inset-0 z-50 grid place-items-center bg-[var(--surface-overlay)] p-4 backdrop-blur-[2px]"
    >
      <div class="motion-panel w-full max-w-md rounded-2xl border border-[var(--state-failed-border)] bg-[var(--bg-panel)] p-5 shadow-[0_40px_80px_-45px_rgba(15,23,42,0.75)]">
        <h3 class="text-base font-semibold text-[var(--state-failed-text)]">{{ t("settings.resetAppTitle") }}</h3>
        <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">{{ t("settings.resetAppConfirm") }}</p>

        <div class="mt-4 flex items-center justify-end gap-2">
          <button
            class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-sm text-[var(--text-main)] hover:bg-[var(--bg-muted)]/70"
            type="button"
            @click="closeResetAppDialog"
          >
            {{ t("settings.resetAppStay") }}
          </button>
          <button
            class="motion-press rounded-lg bg-[var(--state-failed-text)] px-3 py-2 text-sm text-white hover:opacity-95"
            type="button"
            @click="confirmResetApp"
          >
            {{ t("settings.resetAppConfirmAction") }}
          </button>
        </div>
      </div>
    </div>

    <div
      v-if="showUnsavedDialog"
      class="fixed inset-0 z-50 grid place-items-center bg-[var(--surface-overlay)] p-4 backdrop-blur-[2px]"
    >
      <div class="motion-panel w-full max-w-md rounded-2xl border border-[var(--border-muted)] bg-[var(--bg-panel)] p-5 shadow-[0_40px_80px_-45px_rgba(15,23,42,0.75)]">
        <h3 class="text-base font-semibold text-[var(--text-main)]">{{ t("settings.unsavedLeaveTitle") }}</h3>
        <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">{{ t("settings.unsavedConfirm") }}</p>

        <div class="mt-4 flex items-center justify-end gap-2">
          <button
            class="motion-press rounded-lg border border-[var(--border-muted)] bg-[var(--bg-main)] px-3 py-2 text-sm text-[var(--text-main)] hover:bg-[var(--bg-muted)]/70"
            type="button"
            @click="closeUnsavedDialog"
          >
            {{ t("settings.unsavedStay") }}
          </button>
          <button
            class="motion-press rounded-lg bg-[var(--state-failed-text)] px-3 py-2 text-sm text-white hover:opacity-95"
            type="button"
            @click="confirmLeave"
          >
            {{ t("settings.unsavedLeave") }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
