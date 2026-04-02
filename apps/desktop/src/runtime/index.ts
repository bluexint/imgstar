import { createMockRuntime } from "@/runtime/mock";
import { createTauriRuntime } from "@/runtime/tauri";
import type { RuntimeBridge } from "@/types/runtime";

const isTauriHost = (): boolean => {
  if (typeof window === "undefined") {
    return false;
  }

  const candidate = window as unknown as {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  };

  return Boolean(candidate.__TAURI__ || candidate.__TAURI_INTERNALS__);
};

export function createRuntime(): RuntimeBridge {
  const mode = (import.meta.env.VITE_RUNTIME_MODE ?? "auto").toLowerCase();
  if (mode === "tauri") {
    return createTauriRuntime();
  }
  if (mode === "mock") {
    return createMockRuntime();
  }

  if (isTauriHost()) {
    return createTauriRuntime();
  }

  return createMockRuntime();
}
