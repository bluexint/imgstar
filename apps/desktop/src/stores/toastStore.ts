import { defineStore } from "pinia";

export type ToastLevel = "success" | "info" | "warn" | "error";

export interface ToastItem {
  id: number;
  level: ToastLevel;
  message: string;
  traceId?: string;
  sticky: boolean;
  createdAt: number;
  durationMs: number;
}

const MAX_VISIBLE = 3;
const AUTO_HIDE_MS = 3000;
let idSeed = 0;

export const useToastStore = defineStore("toast", {
  state: () => ({
    items: [] as ToastItem[]
  }),
  actions: {
    push(level: ToastLevel, message: string, traceId?: string): void {
      const sticky = level === "error";
      const item: ToastItem = {
        id: ++idSeed,
        level,
        message,
        traceId,
        sticky,
        createdAt: Date.now(),
        durationMs: sticky ? 0 : AUTO_HIDE_MS
      };

      this.items.unshift(item);
      if (this.items.length > MAX_VISIBLE) {
        this.items.pop();
      }

      if (!item.sticky) {
        window.setTimeout(() => {
          this.remove(item.id);
        }, AUTO_HIDE_MS);
      }
    },

    pushSuccess(message: string): void {
      this.push("success", message);
    },

    pushInfo(message: string): void {
      this.push("info", message);
    },

    pushWarn(message: string): void {
      this.push("warn", message);
    },

    pushError(message: string, traceId?: string): void {
      this.push("error", message, traceId);
    },

    remove(id: number): void {
      this.items = this.items.filter((item) => item.id !== id);
    },

    clear(): void {
      this.items = [];
    }
  }
});
