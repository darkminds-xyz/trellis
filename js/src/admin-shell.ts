import "./styles/admin-shell.scss";
import { initSiteSearch } from "./scripts/site-search";
import { initTheme, mountThemeToggle } from "./scripts/theme-toggle";

function bootThemeToggle() {
  initTheme();
  document.querySelectorAll<HTMLElement>("[data-theme-toggle]").forEach((el) => {
    if (!el.dataset.themeToggleMounted) {
      mountThemeToggle(el);
      el.dataset.themeToggleMounted = "true";
    }
  });
}

function bootAdminShell() {
  bootThemeToggle();
  initSiteSearch();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootAdminShell);
} else {
  bootAdminShell();
}
