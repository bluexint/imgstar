import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { useSettingsStore } from "@/stores/settingsStore";

describe("settingsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("switches pristine -> dirty -> saved", async () => {
    const settingsStore = useSettingsStore();

    expect(settingsStore.status).toBe("pristine");

    settingsStore.updateField("accessKey", "ak");
    settingsStore.updateField("secretKey", "sk");
    settingsStore.updateField("endpoint", "https://example.r2.dev");
    settingsStore.updateField("bucket", "demo");
    expect(settingsStore.status).toBe("dirty");

    await settingsStore.save();
    expect(settingsStore.status).toBe("saved");
    expect(settingsStore.lastSavedAt).not.toBe("");
    expect(settingsStore.isConfigured).toBe(true);
  });

  it("resets the app back to defaults", async () => {
    const settingsStore = useSettingsStore();

    settingsStore.updateField("accessKey", "ak");
    settingsStore.updateField("secretKey", "sk");
    settingsStore.updateField("endpoint", "https://example.r2.dev");
    settingsStore.updateField("bucket", "demo");
    await settingsStore.save();

    await settingsStore.resetApp();

    expect(settingsStore.status).toBe("pristine");
    expect(settingsStore.isConfigured).toBe(false);
    expect(settingsStore.draft.accessKey).toBe("");
  });
});
