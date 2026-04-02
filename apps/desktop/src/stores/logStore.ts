import { defineStore } from "pinia";
import type { UploadEvent, UploadEventFilter } from "@imgstar/contracts";
import { api } from "@/services/api";

type LogLevel = UploadEvent["level"];

interface LogFilterState extends UploadEventFilter {
  level?: LogLevel;
}

const DEFAULT_LEVELS: LogLevel[] = ["INFO", "WARN", "ERROR", "DEBUG"];

export const useLogStore = defineStore("logs", {
  state: () => ({
    events: [] as UploadEvent[],
    filter: {
      module: undefined,
      traceId: undefined,
      startAt: undefined,
      endAt: undefined,
      level: undefined
    } as LogFilterState,
    levels: [...DEFAULT_LEVELS],
    live: false
  }),

  actions: {
    async refresh(force = false): Promise<void> {
      if (!this.live && !force) {
        return;
      }

      try {
        const data = await api.listEvents(this.filter);
        this.events = data.filter((event) => this.levels.includes(event.level));
      } catch {
        this.events = [];
      }
    },

    setTraceFocus(traceId: string): void {
      this.filter.traceId = traceId;
      this.levels = ["WARN", "ERROR"];
    },

    setModule(module?: UploadEvent["module"]): void {
      this.filter.module = module;
    },

    setLevel(level?: LogLevel): void {
      this.levels = level ? [level] : [...DEFAULT_LEVELS];
    },

    setLevels(levels: LogLevel[]): void {
      this.levels = levels.length > 0 ? [...levels] : [...DEFAULT_LEVELS];
    },

    setTimeRange(startAt?: string, endAt?: string): void {
      this.filter.startAt = startAt;
      this.filter.endAt = endAt;
    },

    toggleLive(next?: boolean): void {
      this.live = next ?? !this.live;
    },

    async clear(): Promise<void> {
      await api.clearEvents();
      this.clearLocal();
    },

    clearLocal(): void {
      this.events = [];
    }
  }
});
