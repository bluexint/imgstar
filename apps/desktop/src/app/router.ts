import { createRouter, createWebHashHistory } from "vue-router";
import UploadPage from "@/pages/UploadPage.vue";
import PreviewPage from "@/pages/PreviewPage.vue";
import PluginsPage from "@/pages/PluginsPage.vue";
import SettingsPage from "@/pages/SettingsPage.vue";
import DevtoolsPage from "@/pages/DevtoolsPage.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/upload" },
    { path: "/upload", name: "upload", component: UploadPage },
    { path: "/preview", name: "preview", component: PreviewPage },
    { path: "/plugins", name: "plugins", component: PluginsPage },
    { path: "/settings", name: "settings", component: SettingsPage },
    { path: "/devtools", name: "devtools", component: DevtoolsPage }
  ]
});
