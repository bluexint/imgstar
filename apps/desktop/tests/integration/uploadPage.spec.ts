import { createPinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import UploadPage from "@/pages/UploadPage.vue";
import { i18n } from "@/i18n/setup";
import { useUploadStore } from "@/stores/uploadStore";

describe("UploadPage integration", () => {
  it("runs queue file -> upload -> success flow", async () => {
    const pinia = createPinia();
    const wrapper = mount(UploadPage, {
      global: {
        plugins: [pinia, i18n]
      }
    });

    const uploadStore = useUploadStore(pinia);
    uploadStore.hydrated = true;
    uploadStore.addFiles([
      {
        path: "picked/integration-sample.png",
        name: "integration-sample.png",
        size: 128_000,
        mimeType: "image/png"
      }
    ]);

    await flushPromises();
    await wrapper.get('[data-testid="start-upload"]').trigger("click");

    await new Promise((resolve) => setTimeout(resolve, 40));
    await flushPromises();

    const statuses = wrapper.findAll('[data-testid="row-status"]');
    expect(statuses).toHaveLength(1);
    expect(statuses[0].text()).toContain("success");
  });
});
