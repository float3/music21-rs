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
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  document.dispatchEvent(
    new CustomEvent("music21-theme-change", { detail: { theme: resolved } }),
  );
}

function buttonLabel(theme: ResolvedTheme): string {
  return theme === "dark" ? "Light mode" : "Dark mode";
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
  const resolved = resolveTheme(theme);
  button.textContent = buttonLabel(resolved);
  button.setAttribute("aria-label", buttonLabel(resolved));
  button.setAttribute("aria-pressed", String(resolved === "dark"));
}

function installThemeStyles(): void {
  if (document.getElementById("music21-theme-style")) return;

  const style = document.createElement("style");
  style.id = "music21-theme-style";
  style.textContent = `
    .theme-toggle {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-height: 34px;
      padding: 0 12px;
      border: 1px solid var(--line);
      border-radius: 8px;
      background: var(--panel);
      color: var(--accent-strong);
      cursor: pointer;
      font: inherit;
      font-size: 13px;
      font-weight: 800;
      white-space: nowrap;
    }

    .theme-toggle:hover,
    .theme-toggle:focus-visible {
      border-color: var(--accent);
      background: var(--accent-soft);
      outline: none;
    }

    @media (max-width: 760px) {
      .theme-toggle {
        justify-self: start;
      }
    }
  `;
  document.head.appendChild(style);
}

export function setupThemeToggle(): void {
  installThemeStyles();
  let theme = storedTheme();
  applyTheme(theme);

  const button = ensureThemeButton();
  if (!button) return;
  syncButton(button, theme);

  button.addEventListener("click", () => {
    theme = resolveTheme(theme) === "dark" ? "light" : "dark";
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
