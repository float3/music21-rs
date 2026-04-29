const STORAGE_KEY = "music21-rs.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

type ThemeChoice = "light" | "dark" | "system";
type ResolvedTheme = "light" | "dark";

function storedTheme(): ThemeChoice {
  try {
    const value = window.localStorage?.getItem(STORAGE_KEY);
    return value === "light" || value === "dark" || value === "system"
      ? value
      : "system";
  } catch {
    return "system";
  }
}

function storeTheme(theme: ThemeChoice): void {
  try {
    if (theme === "system") {
      window.localStorage?.removeItem(STORAGE_KEY);
    } else {
      window.localStorage?.setItem(STORAGE_KEY, theme);
    }
  } catch {
    // Theme still applies for this page view when storage is unavailable.
  }
}

function systemTheme(): ResolvedTheme {
  const media = window.matchMedia?.(DARK_QUERY);
  return media?.matches ? "dark" : "light";
}

function resolveTheme(theme: ThemeChoice): ResolvedTheme {
  return theme === "system" ? systemTheme() : theme;
}

function applyTheme(theme: ThemeChoice): void {
  const resolved = resolveTheme(theme);
  if (theme === "system") {
    delete document.documentElement.dataset.theme;
    document.documentElement.style.colorScheme = "";
  } else {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.colorScheme = theme;
  }
  document.dispatchEvent(
    new CustomEvent("music21-theme-change", {
      detail: { theme: resolved, choice: theme },
    }),
  );
}

function buttonLabel(theme: ThemeChoice): string {
  if (theme !== "system") return "Use system";
  return resolveTheme(theme) === "dark" ? "Light mode" : "Dark mode";
}

function ensureThemeButton(): HTMLButtonElement | null {
  const header = document.querySelector("header");
  if (!header) return null;

  const existing = header.querySelector<HTMLButtonElement>("[data-theme-toggle]");
  if (existing) return existing;

  const button = document.createElement("button");
  button.type = "button";
  button.className = "theme-toggle";
  button.dataset.themeToggle = "true";
  header.appendChild(button);
  return button;
}

function syncButton(button: HTMLButtonElement, theme: ThemeChoice): void {
  const label = buttonLabel(theme);
  button.textContent = label;
  button.setAttribute("aria-label", label);
  button.setAttribute("aria-pressed", String(theme !== "system"));
}

function nextTheme(theme: ThemeChoice): ThemeChoice {
  if (theme !== "system") return "system";
  return resolveTheme(theme) === "dark" ? "light" : "dark";
}

export function setupThemeToggle(): void {
  let theme = storedTheme();
  applyTheme(theme);

  const button = ensureThemeButton();
  if (!button) return;
  syncButton(button, theme);

  button.addEventListener("click", () => {
    theme = nextTheme(theme);
    storeTheme(theme);
    applyTheme(theme);
    syncButton(button, theme);
  });

  const media = window.matchMedia?.(DARK_QUERY);
  media?.addEventListener?.("change", () => {
    if (storedTheme() !== "system") return;
    theme = "system";
    applyTheme(theme);
    syncButton(button, theme);
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", setupThemeToggle, { once: true });
} else {
  setupThemeToggle();
}
