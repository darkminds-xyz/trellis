import { FileTrieNode, type ContentEntry } from "../../util/fileTrie";
import { resolveRelative, simplifySlug, type FullSlug } from "../../util/path";

declare const fetchData:
  | Promise<Record<string, ContentEntry>>
  | undefined;

type ExplorerStateEntry = { path: string; collapsed: boolean };
type ExplorerOrderStep = "filter" | "map" | "sort";
type ExplorerOptions = {
  folderClickBehavior: "collapse" | "link" | "mixed";
  folderDefaultState: "collapsed" | "open";
  useSavedState: boolean;
  order: ExplorerOrderStep[];
  sortFn?: (a: FileTrieNode<ContentEntry>, b: FileTrieNode<ContentEntry>) => number;
  filterFn?: (node: FileTrieNode<ContentEntry>) => boolean;
  mapFn?: (node: FileTrieNode<ContentEntry>) => void;
};

type DataFns = Partial<
  Pick<ExplorerOptions, "order"> & {
    sortFn: string;
    filterFn: string;
    mapFn: string;
  }
>;

type ContentNode = FileTrieNode<ContentEntry>;

const isElement = (target: EventTarget | null): target is Element =>
  target instanceof Element;

// Trellis renders without the SPA router, so `window.addCleanup` can be undefined.
const registerCleanup: ((fn: () => void) => void) | null =
  typeof window !== "undefined" && typeof window.addCleanup === "function"
    ? window.addCleanup
    : null;

// Always keep folder state as a concrete array so toggle handlers never crash.
let currentExplorerState: ExplorerStateEntry[] = [];

function loadSavedState(useSavedState: boolean): ExplorerStateEntry[] {
  if (!useSavedState) return [];
  const storageTree = localStorage.getItem("fileTree");
  if (!storageTree) return [];
  try {
    return JSON.parse(storageTree);
  } catch (err) {
    console.warn("Explorer: failed to parse saved state", err);
    return [];
  }
}

function findFolderOuter(folderContainer: Element | null): HTMLElement | null {
  if (!folderContainer) return null;
  // Prefer the sibling structure we render server-side.
  const sibling = folderContainer.nextElementSibling;
  if (sibling?.classList.contains("folder-outer")) {
    return sibling as HTMLElement;
  }
  // Fallback: scoped query in case DOM is rewritten.
  return (
    folderContainer.closest("li")?.querySelector(":scope > .folder-outer") ??
    null
  );
}

function attachFolderToggles(explorer: Element, opts: ExplorerOptions): void {
  const containers = explorer.getElementsByClassName("folder-container");
  for (const container of containers) {
    const typedContainer = container as HTMLElement;

    // Chevron toggles always.
    const icon = typedContainer.querySelector<HTMLElement>(".folder-icon");
    if (icon) {
      const iconHandler = (evt: MouseEvent) => {
        evt.stopPropagation();
        toggleFolder(evt, typedContainer);
      };
      icon.addEventListener("click", iconHandler);
      registerCleanup?.(() => icon.removeEventListener("click", iconHandler));
    }

    // When behavior is "collapse" or "mixed", also allow clicking the label/button container.
    if (opts.folderClickBehavior !== "link") {
      const handler = (evt: MouseEvent) => toggleFolder(evt, typedContainer);
      typedContainer.addEventListener("click", handler);
      registerCleanup?.(() =>
        typedContainer.removeEventListener("click", handler)
      );
    }
  }
}
function toggleExplorer(this: HTMLElement, evt: MouseEvent) {
  evt.stopPropagation();
  const nearestExplorer = this.closest<HTMLElement>(".explorer");
  if (!nearestExplorer) return;
  const explorerCollapsed = nearestExplorer.classList.toggle("collapsed");
  nearestExplorer.setAttribute(
    "aria-expanded",
    explorerCollapsed ? "false" : "true"
  );

  if (!explorerCollapsed) {
    // Stop <html> from being scrollable when mobile explorer is open
    document.documentElement.classList.add("mobile-no-scroll");
  } else {
    document.documentElement.classList.remove("mobile-no-scroll");
  }
}

function toggleFolder(evt: MouseEvent, providedContainer?: HTMLElement) {
  evt.stopPropagation();

  const folderContainer =
    providedContainer ??
    (isElement(evt.target)
      ? evt.target.closest<HTMLElement>(".folder-container")
      : null);

  if (!folderContainer) return;

  const childFolderContainer = findFolderOuter(folderContainer);
  if (!childFolderContainer) return;

  const isOpen = !childFolderContainer.classList.contains("open");
  setFolderState(childFolderContainer, !isOpen);
  folderContainer.setAttribute("aria-expanded", isOpen ? "true" : "false");

  // Collapse folder container
  const isCollapsed = !isOpen;

  const folderPath =
    folderContainer.dataset.folderpath ||
    folderContainer.querySelector(".folder-title")?.textContent ||
    "";

  const currentFolderState = currentExplorerState.find(
    (item) => item.path === folderPath
  );
  if (currentFolderState) {
    currentFolderState.collapsed = isCollapsed;
  } else {
    currentExplorerState.push({
      path: folderPath,
      collapsed: isCollapsed,
    });
  }

  const stringifiedFileTree = JSON.stringify(currentExplorerState);
  localStorage.setItem("fileTree", stringifiedFileTree);
}

function createFileNode(currentSlug: FullSlug, node: ContentNode): HTMLLIElement {
  const template = document.getElementById("template-file") as HTMLTemplateElement | null;
  if (!template) throw new Error("template-file not found");
  const clone = template.content.cloneNode(true);
  const li = (clone as DocumentFragment).querySelector("li") as HTMLLIElement;
  const a = li.querySelector("a") as HTMLAnchorElement;
  const nodeSlug = node.slug as FullSlug;
  a.href = resolveRelative(currentSlug, nodeSlug);
  a.dataset.for = nodeSlug;
  a.textContent = node.displayName;

  if (currentSlug === node.slug) {
    a.classList.add("active");
  }
  return li as HTMLLIElement;
}

function createFolderNode(
  currentSlug: FullSlug,
  node: ContentNode,
  opts: ExplorerOptions
): HTMLLIElement {
  const template = document.getElementById("template-folder") as HTMLTemplateElement | null;
  if (!template) throw new Error("template-folder not found");
  const clone = template.content.cloneNode(true);
  const li = (clone as DocumentFragment).querySelector("li") as HTMLLIElement;
  const folderContainer = li.querySelector(".folder-container") as HTMLElement;
  const titleContainer = folderContainer.querySelector("div") as HTMLElement;
  const folderOuter = li.querySelector(".folder-outer") as HTMLElement;
  const ul = folderOuter.querySelector("ul") as HTMLUListElement;

  const folderPath = node.slug as FullSlug;
  folderContainer.dataset.folderpath = folderPath;

  if (opts.folderClickBehavior === "link") {
    // Replace button with link for link behavior
    const button = titleContainer.querySelector(".folder-button") as HTMLElement;
    const a = document.createElement("a");
    a.href = resolveRelative(currentSlug, folderPath);
    a.dataset.for = folderPath;
    a.className = "folder-title";
    a.textContent = node.displayName;
    button.replaceWith(a);
  } else {
    const span = titleContainer.querySelector(".folder-title") as HTMLElement;
    span.textContent = node.displayName;
  }

  // if the saved state is collapsed or the default state is collapsed
  const isCollapsed =
    currentExplorerState.find((item) => item.path === folderPath)?.collapsed ??
    opts.folderDefaultState === "collapsed";

  // if this folder is a prefix of the current path we
  // want to open it anyways
  const simpleFolderPath = simplifySlug(folderPath);
  const folderIsPrefixOfCurrentSlug =
    simpleFolderPath === currentSlug.slice(0, simpleFolderPath.length);

  const shouldBeOpen = !isCollapsed || folderIsPrefixOfCurrentSlug;
  if (shouldBeOpen) {
    folderOuter.classList.add("open");
  }
  folderContainer.setAttribute(
    "aria-expanded",
    shouldBeOpen ? "true" : "false"
  );

  for (const child of node.children) {
    const childNode = child.isFolder
      ? createFolderNode(currentSlug, child, opts)
      : createFileNode(currentSlug, child);
    ul.appendChild(childNode);
  }

  return li as HTMLLIElement;
}

async function setupExplorer(currentSlug: FullSlug) {
  const allExplorers = document.querySelectorAll<HTMLElement>("div.explorer");

  for (const explorer of allExplorers) {
    // Always wire explorer expand/collapse toggles
    const explorerButtons =
      explorer.getElementsByClassName("explorer-toggle");
    for (const button of Array.from(explorerButtons)) {
      (button as HTMLElement).addEventListener("click", toggleExplorer);
      registerCleanup?.(() =>
        (button as HTMLElement).removeEventListener("click", toggleExplorer)
      );
    }

    const dataFns = JSON.parse(explorer.dataset.dataFns || "{}") as DataFns;
    const opts: ExplorerOptions = {
      folderClickBehavior:
        (explorer.dataset.behavior as ExplorerOptions["folderClickBehavior"]) ||
        "collapse",
      folderDefaultState:
        (explorer.dataset.collapsed as ExplorerOptions["folderDefaultState"]) ||
        "collapsed",
      useSavedState: explorer.dataset.savestate === "true",
      order: dataFns.order || ["filter", "map", "sort"],
      sortFn: dataFns.sortFn
        ? (new Function("return " + dataFns.sortFn)() as ExplorerOptions["sortFn"])
        : undefined,
      filterFn: dataFns.filterFn
        ? (new Function("return " + dataFns.filterFn)() as ExplorerOptions["filterFn"])
        : undefined,
      mapFn: dataFns.mapFn
        ? (new Function("return " + dataFns.mapFn)() as ExplorerOptions["mapFn"])
        : undefined,
    };

    // Get folder state from local storage
    const serializedExplorerState = loadSavedState(opts.useSavedState);
    currentExplorerState = [...serializedExplorerState];
    const oldIndex = new Map(
      serializedExplorerState.map((entry) => [entry.path, entry.collapsed])
    );

    // If fetchData is missing (e.g., offline or contentIndex not emitted), keep
    // server-rendered list and still wire toggles.
    if (typeof fetchData === "undefined") {
      applyStateToServerTree(explorer, opts);
      attachFolderToggles(explorer, opts);
      continue;
    }

    let trie: ContentNode;
    try {
      const data = await fetchData;
      const entries = [...Object.entries(data)] as [string, ContentEntry][];
      trie = FileTrieNode.fromEntries(entries);
    } catch (err) {
      console.warn("Explorer: failed to load content index", err);
      applyStateToServerTree(explorer, opts);
      attachFolderToggles(explorer, opts);
      continue;
    }

    // Apply functions in order
    for (const fn of opts.order) {
      switch (fn) {
        case "filter":
          if (opts.filterFn) trie.filter(opts.filterFn);
          break;
        case "map":
          if (opts.mapFn) trie.map(opts.mapFn);
          break;
        case "sort":
          if (opts.sortFn) trie.sort(opts.sortFn);
          break;
      }
    }

    // Get folder paths for state management
    const folderPaths = trie.getFolderPaths();
    currentExplorerState = folderPaths.map((path) => {
      const previousState = oldIndex.get(path);
      return {
        path,
        collapsed:
          previousState === undefined
            ? opts.folderDefaultState === "collapsed"
            : previousState,
      };
    });

    const explorerUl = explorer.querySelector<HTMLUListElement>(".explorer-ul");
    if (!explorerUl) continue;

    // Clear any server-rendered fallback items to avoid duplication
    explorerUl.replaceChildren();

    // Create and insert new content
    explorerUl.replaceChildren();
    const fragment = document.createDocumentFragment();
    for (const child of trie.children) {
      const node = child.isFolder
        ? createFolderNode(currentSlug, child, opts)
        : createFileNode(currentSlug, child);

      fragment.appendChild(node);
    }
    explorerUl.appendChild(fragment);

    // Add overflow sentinel for gradient effect
    const overflowEnd = document.createElement("li");
    overflowEnd.className = "overflow-end";
    explorerUl.appendChild(overflowEnd);

    // restore explorer scrollTop position if it exists
    const scrollTop = sessionStorage.getItem("explorerScrollTop");
    if (scrollTop) {
      explorerUl.scrollTop = parseInt(scrollTop, 10);
    } else {
      // try to scroll to the active element if it exists, but only within the explorer
      const activeElement = explorerUl.querySelector<HTMLElement>(".active");
      if (activeElement) {
        const target =
          activeElement.offsetTop -
          explorerUl.clientHeight / 2 +
          activeElement.clientHeight / 2;
        explorerUl.scrollTop = Math.max(0, target);
      }
    }

    // Set up event handlers
    attachFolderToggles(explorer, opts);
  }
}

function applyStateToServerTree(explorer: Element, opts: ExplorerOptions) {
  const folderContainers = explorer.querySelectorAll<HTMLElement>(".folder-container");
  for (const container of folderContainers) {
    const folderOuter = findFolderOuter(container);
    if (!folderOuter) continue;

    const folderPath =
      container.dataset.folderpath ||
      container.querySelector(".folder-title")?.textContent ||
      "";

    const saved = currentExplorerState.find((item) => item.path === folderPath);
    const shouldCollapse =
      saved?.collapsed ?? opts.folderDefaultState === "collapsed";

    setFolderState(folderOuter, shouldCollapse);
    (container as HTMLElement).setAttribute(
      "aria-expanded",
      shouldCollapse ? "false" : "true"
    );

    if (!saved) {
      currentExplorerState.push({
        path: folderPath,
        collapsed: shouldCollapse,
      });
    }
  }
}

// Hydrate explorer on initial load (SPA and non-SPA) to ensure toggles are wired.
const hydrateExplorer = () => {
  const currentSlug = (window.location.pathname.slice(1) || "index") as FullSlug;
  setupExplorer(currentSlug);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", hydrateExplorer, {
    once: true,
  });
} else {
  hydrateExplorer();
}

document.addEventListener("prenav", async () => {
  // save explorer scrollTop position
  const explorer = document.querySelector(".explorer-ul");
  if (!explorer) return;
  sessionStorage.setItem("explorerScrollTop", explorer.scrollTop.toString());
});

document.addEventListener("nav", async (e: Event) => {
  const detail = (e as CustomEvent<{ url?: FullSlug }>).detail;
  const currentSlug =
    detail?.url ?? ((window.location.pathname.slice(1) || "index") as FullSlug);
  await setupExplorer(currentSlug);

  // if mobile hamburger is visible, collapse by default
  for (const explorer of document.getElementsByClassName("explorer")) {
    const mobileExplorer = explorer.querySelector<HTMLElement>(".mobile-explorer");
    if (!mobileExplorer) return;

    const isVisible =
      typeof (mobileExplorer as HTMLElement & { checkVisibility?: () => boolean })
        .checkVisibility === "function"
        ? (mobileExplorer as HTMLElement & { checkVisibility(): boolean }).checkVisibility()
        : mobileExplorer.offsetParent !== null;

    if (isVisible) {
      explorer.classList.add("collapsed"); 
      explorer.setAttribute("aria-expanded", "false");

      // Allow <html> to be scrollable when mobile explorer is collapsed
      document.documentElement.classList.remove("mobile-no-scroll");
    }

    mobileExplorer.classList.remove("hide-until-loaded");
  }
});

window.addEventListener("resize", function () {
  // Desktop explorer opens by default, and it stays open when the window is resized
  // to mobile screen size. Applies `no-scroll` to <html> in this edge case.
  const explorer = document.querySelector(".explorer");
  if (explorer && !explorer.classList.contains("collapsed")) {
    document.documentElement.classList.add("mobile-no-scroll");
    return;
  }
});

function setFolderState(folderElement: Element, collapsed: boolean): void {
  if (collapsed) {
    folderElement.classList.remove("open");
  } else {
    folderElement.classList.add("open");
  }
}

// Global delegated fallback to capture clicks on nested SVG/polyline nodes.
  (function ensureDelegatedFolderToggle() {
    const handler = (evt: MouseEvent) => {
      if (!isElement(evt.target)) return;
      // Only react to chevrons or folder buttons to avoid hijacking link clicks.
      const icon = evt.target.closest?.(".folder-icon");
      const button = evt.target.closest?.(".folder-button");
      if (!icon && !button) return;
      const folderContainer = evt.target.closest?.(".folder-container") as HTMLElement | null;
      if (!folderContainer) return;
      if (!folderContainer.closest(".explorer")) return;
      toggleFolder(evt, folderContainer);
    };
  document.addEventListener("click", handler);
  registerCleanup?.(() => document.removeEventListener("click", handler));
})();
