type OverlayFolderState = Record<string, boolean>;

const overlayStateKey = "overlayExplorerFolders";

function loadState(): OverlayFolderState {
  try {
    return JSON.parse(localStorage.getItem(overlayStateKey) || "{}") as OverlayFolderState;
  } catch (err) {
    console.warn("Overlay Xplorer: failed to parse saved folder state", err);
    return {};
  }
}

function saveState(state: OverlayFolderState): void {
  localStorage.setItem(overlayStateKey, JSON.stringify(state));
}

function setFolder(entry: HTMLElement, open: boolean): void {
  const selector = entry.dataset.olSelectorFor;
  if (!selector) return;

  const children = document.querySelector<HTMLElement>(
    `[data-ol-children-for="${selector}"]`
  );
  const icon = entry.querySelector<SVGElement>(".ol-folder-icon");
  if (!children || !icon) return;

  children.classList.toggle("open", open);
  icon.classList.toggle("open", open);
  entry.setAttribute("aria-expanded", String(open));
}

export function initOverlayExplorer(): void {
  const button = document.getElementById("overlay-explorer-button") as HTMLButtonElement | null;
  const container = document.getElementById("overlay-explorer-container");
  const panel = document.getElementById("overlay-explorer-content");
  const closeButton = document.querySelector<HTMLButtonElement>("[data-overlay-explorer-close]");
  const list = document.getElementById("overlay-explorer-ul");

  if (!button || !container || !panel || !closeButton || !list) {
    return;
  }

  const savedState = loadState();

  list.querySelectorAll<HTMLElement>(".ol-folder-entry").forEach((entry) => {
    const selector = entry.dataset.olSelectorFor;
    if (!selector || entry.dataset.overlayMounted === "true") return;

    if (selector in savedState) {
      setFolder(entry, savedState[selector]);
    }

    const toggle = (event: Event) => {
      event.preventDefault();
      event.stopPropagation();
      const children = document.querySelector<HTMLElement>(
        `[data-ol-children-for="${selector}"]`
      );
      const open = !children?.classList.contains("open");
      setFolder(entry, open);
      savedState[selector] = open;
      saveState(savedState);
    };

    entry.querySelectorAll<HTMLElement>(".ol-folder-icon, .ol-folder-button").forEach((el) => {
      el.addEventListener("click", toggle);
    });
    entry.dataset.overlayMounted = "true";
  });

  const close = () => {
    container.classList.remove("active");
    container.setAttribute("aria-hidden", "true");
    button.setAttribute("aria-expanded", "false");
    document.documentElement.classList.remove("mobile-no-scroll");
    button.focus();
  };

  const open = () => {
    container.classList.add("active");
    container.setAttribute("aria-hidden", "false");
    button.setAttribute("aria-expanded", "true");
    document.documentElement.classList.add("mobile-no-scroll");
    closeButton.focus();
  };

  if (button.dataset.overlayMounted !== "true") {
    button.addEventListener("click", open);
    closeButton.addEventListener("click", close);
    container.addEventListener("click", (event) => {
      if (event.target === container) {
        close();
      }
    });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && container.classList.contains("active")) {
        close();
      }
    });
    button.dataset.overlayMounted = "true";
  }
}
