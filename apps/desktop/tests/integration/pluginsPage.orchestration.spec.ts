import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import PluginsPage from "@/pages/PluginsPage.vue";
import { i18n } from "@/i18n/setup";
import { usePluginStore } from "@/stores/pluginStore";
import { useToastStore } from "@/stores/toastStore";

describe("PluginsPage orchestration", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows success toast when plugin verification succeeds", async () => {
    vi.useFakeTimers();

    const pinia = createPinia();
    const pluginStore = usePluginStore(pinia);
    const toastStore = useToastStore(pinia);

    const setPluginEnabledSpy = vi
      .spyOn(pluginStore, "setPluginEnabled")
      .mockResolvedValue({
        success: true,
        verified: true
      });

    const successToastSpy = vi.spyOn(toastStore, "pushSuccess");
    const warnToastSpy = vi.spyOn(toastStore, "pushWarn");

    const wrapper = mount(PluginsPage, {
      global: {
        plugins: [pinia, i18n]
      }
    });

    await flushPromises();

    const toggleButtons = wrapper.findAll("article > div > button[type='button']");
    expect(toggleButtons.length).toBeGreaterThan(0);

    await toggleButtons[0].trigger("click");
    await flushPromises();

    expect(setPluginEnabledSpy).toHaveBeenCalledWith(
      pluginStore.plugins[0].id,
      true
    );
    expect(successToastSpy).toHaveBeenCalledTimes(1);
    expect(warnToastSpy).not.toHaveBeenCalled();

    wrapper.unmount();
  });

  it("shows warning toast when plugin verification fails", async () => {
    vi.useFakeTimers();

    const pinia = createPinia();
    const pluginStore = usePluginStore(pinia);
    const toastStore = useToastStore(pinia);

    const setPluginEnabledSpy = vi
      .spyOn(pluginStore, "setPluginEnabled")
      .mockResolvedValue({
        success: false,
        verified: false,
        reason: "SIGNATURE_VERIFY_FAILED"
      });

    const successToastSpy = vi.spyOn(toastStore, "pushSuccess");
    const warnToastSpy = vi.spyOn(toastStore, "pushWarn");

    const wrapper = mount(PluginsPage, {
      global: {
        plugins: [pinia, i18n]
      }
    });

    await flushPromises();

    const toggleButtons = wrapper.findAll("article > div > button[type='button']");
    expect(toggleButtons.length).toBeGreaterThan(0);

    await toggleButtons[0].trigger("click");
    await flushPromises();

    expect(setPluginEnabledSpy).toHaveBeenCalledWith(
      pluginStore.plugins[0].id,
      true
    );
    expect(successToastSpy).not.toHaveBeenCalled();
    expect(warnToastSpy).toHaveBeenCalledTimes(1);

    wrapper.unmount();
  });
});
