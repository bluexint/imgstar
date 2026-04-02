<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useToastStore } from "@/stores/toastStore";

const { t } = useI18n();
const toastStore = useToastStore();
const router = useRouter();

const colorClass = (level: string): string => {
  if (level === "success") {
    return "border-[var(--toast-success-border)] bg-[var(--toast-success-bg)] text-[var(--toast-success-text)]";
  }
  if (level === "info") {
    return "border-[var(--toast-info-border)] bg-[var(--toast-info-bg)] text-[var(--toast-info-text)]";
  }
  if (level === "warn") {
    return "border-[var(--toast-warn-border)] bg-[var(--toast-warn-bg)] text-[var(--toast-warn-text)]";
  }
  return "border-[var(--toast-error-border)] bg-[var(--toast-error-bg)] text-[var(--toast-error-text)]";
};

const timerBarClass = (level: string): string => {
  if (level === "success") {
    return "bg-[var(--toast-success-bar)]";
  }
  if (level === "info") {
    return "bg-[var(--toast-info-bar)]";
  }
  if (level === "warn") {
    return "bg-[var(--toast-warn-bar)]";
  }
  return "bg-[var(--toast-error-bar)]";
};

const timerStyle = (durationMs: number): Record<string, string> => ({
  animationDuration: `${durationMs}ms`
});

const toasts = computed(() => toastStore.items);

const jumpToDetails = async (traceId?: string): Promise<void> => {
  if (!traceId) {
    return;
  }

  await router.push({
    name: "devtools",
    query: {
      traceId,
      levels: "WARN,ERROR"
    }
  });
};
</script>

<template>
  <div class="pointer-events-none fixed right-3 top-3 z-50 w-[19rem] max-w-[calc(100vw-1.5rem)]">
    <TransitionGroup name="toast-stack" tag="div" class="flex flex-col gap-2">
      <article
        v-for="item in toasts"
        :key="item.id"
        :class="colorClass(item.level)"
        class="motion-panel pointer-events-auto relative overflow-hidden rounded-xl border p-2.5 text-sm shadow-[0_18px_40px_-24px_rgba(15,23,42,0.55)] backdrop-blur-[2px]"
      >
        <div
          v-if="item.durationMs > 0"
          :class="timerBarClass(item.level)"
          :style="timerStyle(item.durationMs)"
          class="toast-timebar absolute inset-x-0 top-0 h-0.5"
        ></div>

        <button
          class="motion-press absolute right-1.5 top-1.5 grid h-5 w-5 place-items-center rounded-full text-[var(--text-muted)] hover:bg-black/10 hover:text-[var(--text-main)]"
          type="button"
          @click="toastStore.remove(item.id)"
        >
          <svg
            aria-hidden="true"
            class="h-3.5 w-3.5"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M18 6 6 18" />
            <path d="m6 6 12 12" />
          </svg>
        </button>

        <p class="break-words pr-6 text-[13px] leading-[1.3rem]">{{ item.message }}</p>

        <div class="mt-2.5 flex items-center justify-end gap-2 text-xs">
          <button
            v-if="item.level === 'error' && item.traceId"
            class="motion-press rounded-md border border-[var(--border-muted)] bg-[var(--bg-panel)]/80 px-2 py-1 font-medium text-[var(--text-main)] hover:bg-[var(--bg-panel)]"
            type="button"
            @click="jumpToDetails(item.traceId)"
          >
            {{ t("toast.viewDetails") }}
          </button>
        </div>
      </article>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-stack-enter-active,
.toast-stack-leave-active {
  transition: all 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.toast-stack-enter-from,
.toast-stack-leave-to {
  opacity: 0;
  transform: translateY(-8px) scale(0.96);
}

.toast-stack-move {
  transition: transform 220ms ease;
}

.toast-timebar {
  transform-origin: left;
  animation-name: toast-timebar;
  animation-timing-function: linear;
  animation-fill-mode: forwards;
}

@keyframes toast-timebar {
  from {
    transform: scaleX(1);
    opacity: 0.9;
  }

  to {
    transform: scaleX(0);
    opacity: 0.2;
  }
}
</style>
