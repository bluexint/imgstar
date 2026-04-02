import { defineStore } from "pinia";
import type { PluginConfig } from "@imgstar/contracts";
import { api } from "@/services/api";
import {
  DEFAULT_IMAGE_OPTIONS,
  type ImageProcessingOptions
} from "@/types/imageProcessing";

type PluginStatus = "disabled" | "enabled" | "error";

interface PluginItem {
  id: string;
  nameKey: string;
  descriptionKey: string;
  signerSource: string;
  status: PluginStatus;
  hookType: PluginConfig["hookType"];
  stage: PluginConfig["stage"];
  priority: number;
  error?: string;
}

export interface PluginEnableResult {
  success: boolean;
  verified: boolean;
  reason?: string;
}

const toPluginConfig = (plugin: PluginItem): PluginConfig => ({
  id: plugin.id,
  enabled: plugin.status === "enabled",
  hookType: plugin.hookType,
  stage: plugin.stage,
  priority: plugin.priority
});

export const usePluginStore = defineStore("plugin", {
  state: () => ({
    imageOptions: { ...DEFAULT_IMAGE_OPTIONS },
    plugins: [
      {
        id: "image-compress",
        nameKey: "plugins.imageCompressName",
        descriptionKey: "plugins.imageCompressDescription",
        signerSource: "imgstar-official",
        status: "disabled",
        hookType: "upload",
        stage: "pre_key",
        priority: 1
      },
      {
        id: "hidden-watermark",
        nameKey: "plugins.hiddenWatermarkName",
        descriptionKey: "plugins.hiddenWatermarkDescription",
        signerSource: "imgstar-official",
        status: "disabled",
        hookType: "upload",
        stage: "pre_key",
        priority: 2
      }
    ] as PluginItem[]
  }),

  getters: {
    activeUploadChain: (state): PluginConfig[] =>
      state.plugins
        .map(toPluginConfig)
        .filter((plugin) => plugin.enabled && plugin.hookType === "upload")
        .sort((left, right) => left.priority - right.priority)
  },

  actions: {
    async setPluginEnabled(id: string, enabled: boolean): Promise<PluginEnableResult> {
      const plugin = this.plugins.find((item) => item.id === id);
      if (!plugin) {
        return { success: false, verified: false, reason: "not_found" };
      }

      if (!enabled) {
        plugin.status = "disabled";
        plugin.error = undefined;
        return { success: true, verified: false };
      }

      const verify = await api.verifyPlugin(id, plugin.signerSource);
      if (verify.verified) {
        plugin.status = "enabled";
        plugin.error = undefined;
        return { success: true, verified: true };
      }

      plugin.status = "disabled";
      plugin.error = verify.reason ?? "SIGNATURE_VERIFY_FAILED";
      return {
        success: false,
        verified: false,
        reason: plugin.error
      };
    },

    setImageOption<K extends keyof ImageProcessingOptions>(
      key: K,
      value: ImageProcessingOptions[K]
    ): void {
      this.imageOptions[key] = value;
    }
  }
});
