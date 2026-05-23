const STORAGE_KEY = "saved-theme";
const LEGACY_STORAGE_KEY = "theme";
const THEME_ATTRIBUTE = "saved-theme";
type Theme = "light" | "dark";

function systemTheme(): Theme {
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function storedTheme(): Theme | null {
  const value =
    localStorage.getItem(STORAGE_KEY) ||
    localStorage.getItem(LEGACY_STORAGE_KEY);
  return value === "dark" || value === "light" ? value : null;
}

function currentTheme(): Theme {
  return document.documentElement.getAttribute(THEME_ATTRIBUTE) === "dark"
    ? "dark"
    : "light";
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.setAttribute(THEME_ATTRIBUTE, theme);
  root.style.colorScheme = theme;
  localStorage.setItem(STORAGE_KEY, theme);
  localStorage.removeItem(LEGACY_STORAGE_KEY);
  document.dispatchEvent(new CustomEvent("themechange", { detail: { theme } }));
}

export function initTheme() {
  applyTheme(storedTheme() || systemTheme());
}

export function mountThemeToggle(target: HTMLElement = document.body) {
  const button = document.getElementById("theme-toggle") as HTMLButtonElement;

  const syncButtonState = () => {
    const dark = currentTheme() === "dark";
    button.setAttribute("aria-pressed", String(dark));
    button.title = dark ? "Use light theme" : "Use dark theme";
  };

  document.addEventListener("themechange", syncButtonState);

  button.onclick = () => {
    applyTheme(currentTheme() === "dark" ? "light" : "dark");
    syncButtonState();
  };

  syncButtonState();
  target.append(button);
  return button;
}
