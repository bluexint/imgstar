<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();

type NavLink = {
  name: string;
  path: string;
  label: string;
  icon: string[];
};

const links: NavLink[] = [
  {
    name: "upload",
    path: "/upload",
    label: "nav.upload",
    icon: [
      "M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5",
      "M16.5 12 12 7.5 7.5 12",
      "M12 7.5V18"
    ]
  },
  {
    name: "preview",
    path: "/preview",
    label: "nav.preview",
    icon: [
      "M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6-10-6-10-6z",
      "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6"
    ]
  },
  {
    name: "plugins",
    path: "/plugins",
    label: "nav.plugins",
    icon: [
      "M4 9h16",
      "M8 9V4",
      "M16 9V4",
      "M12 9v8",
      "M9 20h6"
    ]
  },
  {
    name: "settings",
    path: "/settings",
    label: "nav.settings",
    icon: [
      "M12 8.75a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5z",
      "M19.4 15a1 1 0 0 0 .2 1.1l.1.1a2 2 0 1 1-2.83 2.83l-.1-.1a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.92V20a2 2 0 1 1-4 0v-.14a1 1 0 0 0-.6-.92 1 1 0 0 0-1.1.2l-.1.1a2 2 0 1 1-2.83-2.83l.1-.1a1 1 0 0 0 .2-1.1 1 1 0 0 0-.92-.6H4a2 2 0 1 1 0-4h.14a1 1 0 0 0 .92-.6 1 1 0 0 0-.2-1.1l-.1-.1a2 2 0 1 1 2.83-2.83l.1.1a1 1 0 0 0 1.1.2 1 1 0 0 0 .6-.92V4a2 2 0 1 1 4 0v.14a1 1 0 0 0 .6.92 1 1 0 0 0 1.1-.2l.1-.1a2 2 0 1 1 2.83 2.83l-.1.1a1 1 0 0 0-.2 1.1 1 1 0 0 0 .92.6H20a2 2 0 1 1 0 4h-.14a1 1 0 0 0-.92.6z"
    ]
  },
  {
    name: "devtools",
    path: "/devtools",
    label: "nav.devtools",
    icon: [
      "M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z",
      "m7 10 2 2-2 2",
      "M12 14h5"
    ]
  }
];
</script>

<template>
  <aside class="flex h-full flex-col items-center gap-2.5 bg-[var(--bg-panel)] py-3">
    <RouterLink
      v-for="item in links"
      :key="item.name"
      :to="item.path"
      class="group flex w-12 flex-col items-center gap-1 rounded-xl border border-transparent px-1 py-2 text-[10px] font-medium text-[var(--text-main)] transition duration-200 hover:border-[var(--accent)]/50 hover:bg-[var(--bg-muted)]/60 hover:text-[var(--accent)]"
      active-class="border-[var(--accent)]/60 bg-[var(--bg-muted)] text-[var(--accent)] shadow-[0_10px_28px_-18px_var(--accent)]"
    >
      <span class="grid h-8 w-8 place-items-center rounded-lg bg-[var(--bg-muted)] transition duration-200 group-hover:scale-[1.06] group-hover:bg-[var(--accent)]/12">
        <svg
          class="h-[18px] w-[18px]"
          fill="none"
          stroke="currentColor"
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.8"
          viewBox="0 0 24 24"
        >
          <path v-for="(path, index) in item.icon" :key="`${item.name}-${index}`" :d="path" />
        </svg>
      </span>
      <span class="text-center leading-3">{{ t(item.label) }}</span>
    </RouterLink>
  </aside>
</template>
