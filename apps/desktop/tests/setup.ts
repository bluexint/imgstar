import { beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { resetRuntime } from "@/services/api";

beforeEach(() => {
  window.localStorage.clear();
  setActivePinia(createPinia());
  resetRuntime();
});
