export type ThemeMode = "light" | "dark";

const LIGHT_THEME: Record<string, string> = {
  "--bg-main": "#f8fafc",
  "--bg-panel": "#ffffff",
  "--bg-muted": "#e2e8f0",
  "--text-main": "#1e293b",
  "--text-muted": "#475569",
  "--text-placeholder": "#64748b",
  "--accent": "#3b82f6",
  "--accent-strong": "#1d4ed8",
  "--border-muted": "#cbd5e1",
  "--surface-overlay": "rgba(255, 255, 255, 0.94)",
  "--state-reserved-bg": "#fffbeb",
  "--state-reserved-text": "#b45309",
  "--state-reserved-border": "#f59e0b",
  "--state-active-bg": "#ecfdf5",
  "--state-active-text": "#047857",
  "--state-active-border": "#10b981",
  "--state-recycling-bg": "#eff6ff",
  "--state-recycling-text": "#0369a1",
  "--state-recycling-border": "#0ea5e9",
  "--state-recycled-bg": "#f1f5f9",
  "--state-recycled-text": "#475569",
  "--state-recycled-border": "#94a3b8",
  "--state-failed-bg": "#fef2f2",
  "--state-failed-text": "#b91c1c",
  "--state-failed-border": "#ef4444",
  "--toast-success-bg": "rgba(236, 253, 245, 0.96)",
  "--toast-success-text": "#064e3b",
  "--toast-success-border": "rgba(52, 211, 153, 0.8)",
  "--toast-success-bar": "#10b981",
  "--toast-info-bg": "rgba(239, 246, 255, 0.96)",
  "--toast-info-text": "#0f172a",
  "--toast-info-border": "rgba(125, 211, 252, 0.8)",
  "--toast-info-bar": "#0ea5e9",
  "--toast-warn-bg": "rgba(255, 251, 235, 0.96)",
  "--toast-warn-text": "#713f12",
  "--toast-warn-border": "rgba(251, 191, 36, 0.82)",
  "--toast-warn-bar": "#f59e0b",
  "--toast-error-bg": "rgba(254, 242, 242, 0.96)",
  "--toast-error-text": "#7f1d1d",
  "--toast-error-border": "rgba(252, 165, 165, 0.82)",
  "--toast-error-bar": "#ef4444"
};

const DARK_THEME: Record<string, string> = {
  "--bg-main": "#0b1220",
  "--bg-panel": "#111827",
  "--bg-muted": "#1f2937",
  "--text-main": "#f8fafc",
  "--text-muted": "#d1d5db",
  "--text-placeholder": "#94a3b8",
  "--accent": "#60a5fa",
  "--accent-strong": "#93c5fd",
  "--border-muted": "#334155",
  "--surface-overlay": "rgba(15, 23, 42, 0.94)",
  "--state-reserved-bg": "rgba(251, 191, 36, 0.16)",
  "--state-reserved-text": "#fbbf24",
  "--state-reserved-border": "rgba(251, 191, 36, 0.45)",
  "--state-active-bg": "rgba(16, 185, 129, 0.16)",
  "--state-active-text": "#34d399",
  "--state-active-border": "rgba(16, 185, 129, 0.45)",
  "--state-recycling-bg": "rgba(56, 189, 248, 0.16)",
  "--state-recycling-text": "#7dd3fc",
  "--state-recycling-border": "rgba(56, 189, 248, 0.45)",
  "--state-recycled-bg": "rgba(148, 163, 184, 0.16)",
  "--state-recycled-text": "#cbd5e1",
  "--state-recycled-border": "rgba(148, 163, 184, 0.45)",
  "--state-failed-bg": "rgba(248, 113, 113, 0.16)",
  "--state-failed-text": "#fca5a5",
  "--state-failed-border": "rgba(248, 113, 113, 0.45)",
  "--toast-success-bg": "rgba(6, 95, 70, 0.28)",
  "--toast-success-text": "#d1fae5",
  "--toast-success-border": "rgba(16, 185, 129, 0.45)",
  "--toast-success-bar": "#34d399",
  "--toast-info-bg": "rgba(30, 41, 59, 0.92)",
  "--toast-info-text": "#e2e8f0",
  "--toast-info-border": "rgba(125, 211, 252, 0.45)",
  "--toast-info-bar": "#38bdf8",
  "--toast-warn-bg": "rgba(120, 53, 15, 0.28)",
  "--toast-warn-text": "#fef3c7",
  "--toast-warn-border": "rgba(251, 191, 36, 0.45)",
  "--toast-warn-bar": "#f59e0b",
  "--toast-error-bg": "rgba(127, 29, 29, 0.3)",
  "--toast-error-text": "#fee2e2",
  "--toast-error-border": "rgba(248, 113, 113, 0.45)",
  "--toast-error-bar": "#f87171"
};

const applyVariables = (theme: Record<string, string>): void => {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(theme)) {
    root.style.setProperty(key, value);
  }
};

export function applyTheme(mode: ThemeMode): ThemeMode {
  const root = document.documentElement;
  root.dataset.theme = mode;
  root.style.colorScheme = mode;
  applyVariables(mode === "dark" ? DARK_THEME : LIGHT_THEME);
  return mode;
}

export function initializeTheme(): ThemeMode {
  return applyTheme("light");
}

export function getTheme(): ThemeMode {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

export function toggleTheme(): ThemeMode {
  return applyTheme(getTheme() === "light" ? "dark" : "light");
}
