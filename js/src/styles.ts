import "./styles/base.scss";
import "./styles/custom.scss";
import { initTheme, mountThemeToggle } from "./scripts/theme-toggle";

function bootThemeToggle() {
  initTheme();
  document.querySelectorAll<HTMLElement>("[data-theme-toggle]").forEach((el) => {
    if (!el.querySelector(".theme-toggle")) {
      mountThemeToggle(el);
    }
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootThemeToggle);
} else {
  bootThemeToggle();
}
