import "./styles/base.scss";
import "./styles/custom.scss";
import "./scripts/xplorer.inline";
import { initCallouts } from "./scripts/callouts";
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

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    bootThemeToggle();
    initCallouts();
  });
} else {
  bootThemeToggle();
  initCallouts();
}
