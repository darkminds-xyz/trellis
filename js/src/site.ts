import "./styles/site.scss";
import "./scripts/xplorer.inline";
import { initCallouts } from "./scripts/callouts";
import { initClipboardButtons } from "./scripts/clipboard.inline";
import { initOverlayExplorer } from "./scripts/overlay-explorer";
import { initPopovers } from "./scripts/popover.inline";
import { initSiteSearch } from "./scripts/site-search";
import { initTheme, mountThemeToggle } from "./scripts/theme-toggle";
import { initToc } from "./scripts/toc.inline";

function bootThemeToggle() {
  initTheme();
  document.querySelectorAll<HTMLElement>("[data-theme-toggle]").forEach((el) => {
    if (!el.dataset.themeToggleMounted) {
      mountThemeToggle(el);
      el.dataset.themeToggleMounted = "true";
    }
  });
}

function bootSite() {
  bootThemeToggle();
  initOverlayExplorer();
  initSiteSearch();
  initPopovers();
  initCallouts();
  initClipboardButtons();
  initToc();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootSite);
} else {
  bootSite();
}

document.addEventListener("nav", () => {
  initClipboardButtons();
  initToc();
});
