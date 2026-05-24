import { registerCleanup } from "../utils/cleanup";

type FolderState = {
  path: string;
  collapsed: boolean;
};

type ExplorerOptions = {
  folderClickBehavior: "collapse" | "link" | "mixed";
  folderDefaultState: "collapsed" | "open";
  useSavedState: boolean;
};

let currentExplorerState: FolderState[] = [];

function loadSavedState(useSavedState: boolean): FolderState[] {
  if (!useSavedState) return [];
  const storageTree = localStorage.getItem("fileTree");
  if (!storageTree) return [];

  try {
    return JSON.parse(storageTree) as FolderState[];
  } catch (err) {
    console.warn("Xplorer: failed to parse saved folder state", err);
    return [];
  }
}

function saveState(useSavedState: boolean): void {
  if (!useSavedState) return;
  localStorage.setItem("fileTree", JSON.stringify(currentExplorerState));
}

function folderOuterFor(container: HTMLElement): HTMLElement | null {
  const sibling = container.nextElementSibling;
  if (sibling?.classList.contains("folder-outer")) {
    return sibling as HTMLElement;
  }

  return (
    container.closest("li")?.querySelector<HTMLElement>(":scope > .folder-outer") ??
    null
  );
}

function setFolderState(
  folderContainer: HTMLElement,
  collapsed: boolean
): void {
  const folderOuter = folderOuterFor(folderContainer);
  if (!folderOuter) return;

  folderOuter.classList.toggle("open", !collapsed);
  folderContainer.setAttribute("aria-expanded", collapsed ? "false" : "true");
}

function toggleFolder(
  folderContainer: HTMLElement,
  opts: ExplorerOptions,
  evt?: Event
): void {
  evt?.stopPropagation();

  const folderOuter = folderOuterFor(folderContainer);
  if (!folderOuter) return;

  const collapsed = folderOuter.classList.contains("open");
  setFolderState(folderContainer, collapsed);

  const path =
    folderContainer.dataset.folderpath ||
    folderContainer.querySelector(".folder-title")?.textContent ||
    "";
  if (!path) return;

  const state = currentExplorerState.find((entry) => entry.path === path);
  if (state) {
    state.collapsed = collapsed;
  } else {
    currentExplorerState.push({ path, collapsed });
  }

  saveState(opts.useSavedState);
}

function toggleExplorer(button: HTMLElement, evt: Event): void {
  evt.stopPropagation();

  const explorer = button.closest<HTMLElement>(".explorer");
  if (!explorer) return;

  const collapsed = explorer.classList.toggle("collapsed");
  explorer.setAttribute("aria-expanded", collapsed ? "false" : "true");
  button.setAttribute("aria-expanded", collapsed ? "false" : "true");

  if (collapsed) {
    document.documentElement.classList.remove("mobile-no-scroll");
  } else {
    document.documentElement.classList.add("mobile-no-scroll");
  }
}

function folderContainsActivePage(folderContainer: HTMLElement): boolean {
  const folderOuter = folderOuterFor(folderContainer);
  return Boolean(folderOuter?.querySelector(".active, [aria-current='page']"));
}

function mountExplorer(explorer: HTMLElement): void {
  const dataFns = explorer.dataset;
  const opts: ExplorerOptions = {
    folderClickBehavior:
      (dataFns.behavior as ExplorerOptions["folderClickBehavior"]) || "collapse",
    folderDefaultState:
      (dataFns.collapsed as ExplorerOptions["folderDefaultState"]) || "collapsed",
    useSavedState: dataFns.savestate === "true",
  };

  const savedState = loadSavedState(opts.useSavedState);
  const savedIndex = new Map(savedState.map((entry) => [entry.path, entry.collapsed]));
  currentExplorerState = [];

  explorer.querySelectorAll<HTMLElement>(".folder-container").forEach((container) => {
    const path =
      container.dataset.folderpath ||
      container.querySelector(".folder-title")?.textContent ||
      "";
    if (!path) return;

    const collapsed =
      savedIndex.get(path) ?? opts.folderDefaultState === "collapsed";
    const shouldCollapse = folderContainsActivePage(container) ? false : collapsed;

    currentExplorerState.push({ path, collapsed: shouldCollapse });
    setFolderState(container, shouldCollapse);

    const icon = container.querySelector<HTMLElement>(".folder-icon");
    if (icon && icon.dataset.xplorerMounted !== "true") {
      const handler = (evt: Event) => toggleFolder(container, opts, evt);
      icon.addEventListener("click", handler);
      registerCleanup?.(() => icon.removeEventListener("click", handler));
      icon.dataset.xplorerMounted = "true";
    }

    const button = container.querySelector<HTMLElement>(".folder-button");
    if (
      button &&
      opts.folderClickBehavior !== "link" &&
      button.dataset.xplorerMounted !== "true"
    ) {
      const handler = (evt: Event) => toggleFolder(container, opts, evt);
      button.addEventListener("click", handler);
      registerCleanup?.(() => button.removeEventListener("click", handler));
      button.dataset.xplorerMounted = "true";
    }
  });

  explorer.querySelectorAll<HTMLElement>(".explorer-toggle").forEach((button) => {
    if (button.dataset.xplorerMounted === "true") return;

    const handler = (evt: Event) => toggleExplorer(button, evt);
    button.addEventListener("click", handler);
    registerCleanup?.(() => button.removeEventListener("click", handler));
    button.dataset.xplorerMounted = "true";
  });
}

function setupXplorer(): void {
  document
    .querySelectorAll<HTMLElement>(".explorer")
    .forEach((explorer) => mountExplorer(explorer));
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", setupXplorer, { once: true });
} else {
  setupXplorer();
}

document.addEventListener("nav", setupXplorer);
