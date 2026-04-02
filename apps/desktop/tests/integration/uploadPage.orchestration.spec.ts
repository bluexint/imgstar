import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import UploadPage from "@/pages/UploadPage.vue";
import { i18n } from "@/i18n/setup";
import { useLogStore } from "@/stores/logStore";
import { usePluginStore } from "@/stores/pluginStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useUploadStore } from "@/stores/uploadStore";

describe("UploadPage orchestration", () => {
  it("passes plugin options and refreshes dependent stores after start", async () => {
    const pinia = createPinia();
    const uploadStore = useUploadStore(pinia);
    const pluginStore = usePluginStore(pinia);
    const logStore = useLogStore(pinia);
    const settingsStore = useSettingsStore(pinia);
    uploadStore.hydrated = true;

    uploadStore.addFiles([
      {
        path: "picked/orchestration.png",
        name: "orchestration.png",
        size: 4096,
        mimeType: "image/png"
      }
    ]);

    pluginStore.plugins[0].status = "enabled";
    pluginStore.setImageOption("quality", 0.72);

    const startSpy = vi
      .spyOn(uploadStore, "startQueuedUploads")
      .mockResolvedValue();
    const logRefreshSpy = vi.spyOn(logStore, "refresh").mockResolvedValue();
    const pingRefreshSpy = vi
      .spyOn(settingsStore, "refreshPing")
      .mockResolvedValue();

    const wrapper = mount(UploadPage, {
      global: {
        plugins: [pinia, i18n]
      }
    });

    await wrapper.get('[data-testid="start-upload"]').trigger("click");
    await flushPromises();

    expect(startSpy).toHaveBeenCalledTimes(1);
    expect(startSpy).toHaveBeenCalledWith({
      pluginChain: pluginStore.activeUploadChain,
      imageOptions: pluginStore.imageOptions
    });
    expect(logRefreshSpy).toHaveBeenCalledWith(true);
    expect(pingRefreshSpy).toHaveBeenCalledWith(true);
  });
});
