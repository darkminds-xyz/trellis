import { FileTrieNode, type ContentEntry } from "../../util/fileTrie";
import { resolveRelative, simplifySlug, type FullSlug } from "../../util/path";
import { registerEscapeHandler } from "./util";

declare const fetchData: Promise<Record<string, ContentEntry>> | undefined;

type ExplorerClickBehavior = "collapse" | "link" | "mixed";
type ExplorerDefaultState = "collapsed" | "open";

type ExplorerOptions = {
  folderClickBehavior: ExplorerClickBehavior;
  folderDefaultState: ExplorerDefaultState;
  useSavedState: boolean;
};

type ContentNode = FileTrieNode<ContentEntry>;

// Trellis renders without the SPA router, so `window.addCleanup` can be undefined.
const registerCleanup =
  typeof window !== "undefined" &&
  typeof (window as any).addCleanup === "function"
    ? (window as any).addCleanup
    : null;

function setFolder(folderPath: string, open: boolean) {
  const children = document.querySelector(
    `[data-ol-children-for='${folderPath}']`
  );
  const entry = document.querySelector(
    `[data-ol-selector-for='${folderPath}']`
  );
  if (!children || !entry) return;
  const icon = entry.querySelector("svg");
  if (!icon) return;
  children.classList.toggle("open", open);
  icon.classList.toggle("open", open);
}

function sortChildren(children: ContentNode[]): ContentNode[] {
  return [...children].sort((a, b) => {
    if (a.isFolder && !b.isFolder) return -1;
    if (!a.isFolder && b.isFolder) return 1;
    return a.displayName.localeCompare(b.displayName, undefined, {
      numeric: true,
      sensitivity: "base",
    });
  });
}

function buildNode(
  node: ContentNode,
  currentSlug: FullSlug,
  opts: ExplorerOptions,
  folderPath = ""
): HTMLLIElement {
  const li = document.createElement("li");

  if (node.isFolder) {
    const fullPath = (node.slug || folderPath || "index") as FullSlug;
    const selectorPath = fullPath.endsWith("/")
      ? fullPath + "index"
      : `${fullPath}/index`;

    const entry = document.createElement("div");
    entry.className = "ol-folder-entry";
    entry.dataset.olSelectorFor = selectorPath;

    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    icon.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    icon.setAttribute("width", "12");
    icon.setAttribute("height", "12");
    icon.setAttribute("viewBox", "5 8 14 8");
    icon.setAttribute("fill", "none");
    icon.setAttribute("stroke", "currentColor");
    icon.setAttribute("stroke-width", "2");
    icon.setAttribute("stroke-linecap", "round");
    icon.setAttribute("stroke-linejoin", "round");
    icon.classList.add("ol-folder-icon");
    icon.innerHTML = '<polyline points="6 9 12 15 18 9"></polyline>';
    entry.appendChild(icon);

    if (opts.folderClickBehavior === "link") {
      const a = document.createElement("a");
      a.href = resolveRelative(currentSlug, fullPath);
      a.className = "ol-folder-title";
      a.textContent = node.displayName;
      entry.appendChild(a);
    } else {
      const btn = document.createElement("button");
      btn.className = "ol-folder-button";
      btn.type = "button";
      const span = document.createElement("span");
      span.className = "ol-folder-title";
      span.textContent = node.displayName;
      btn.appendChild(span);
      entry.appendChild(btn);

      if (opts.folderClickBehavior === "mixed") {
        const link = document.createElement("a");
        link.href = resolveRelative(currentSlug, fullPath);
        link.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="12" viewBox="0 4 21 15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="0 9 18 9"></polyline><polyline points="0 15 18 15"></polyline><polyline points="15 6 21 12 15 18"></polyline></svg>`;
        entry.appendChild(link);
      }
    }

    li.appendChild(entry);

    const outer = document.createElement("div");
    outer.className = "ol-folder-outer";
    outer.dataset.olChildrenFor = selectorPath;
    const ul = document.createElement("ul");
    const hasName = node.slugSegment !== "";
    ul.style.paddingLeft = hasName ? "1.4rem" : "0";

    const sortedChildren = sortChildren(node.children as ContentNode[]);
    for (const child of sortedChildren) {
      ul.appendChild(buildNode(child, currentSlug, opts, fullPath));
    }

    outer.appendChild(ul);
    li.appendChild(outer);

    const shouldOpen =
      opts.folderDefaultState === "open" ||
      simplifySlug(fullPath as FullSlug).startsWith(currentSlug);
    if (shouldOpen) {
      outer.classList.add("open");
      icon.classList.add("open");
    }
  } else {
    const a = document.createElement("a");
    a.href = resolveRelative(currentSlug, node.slug as FullSlug);
    a.textContent = node.displayName;
    li.appendChild(a);
  }

  return li;
}

function attachFolderToggles(root: Element, useSaveState: boolean) {
  const map: Map<string, boolean> = useSaveState
    ? new Map(
        JSON.parse(localStorage.getItem("olFileTree") || "[]") as [
          string,
          boolean
        ][]
      )
    : new Map<string, boolean>();

  const save = () => {
    if (useSaveState) {
      localStorage.setItem(
        "olFileTree",
        JSON.stringify(Array.from(map.entries()))
      );
    }
  };

  const toggle = (evt: Event) => {
    const mouseEvt = evt as MouseEvent;
    mouseEvt.stopPropagation();
    const target = mouseEvt.currentTarget as HTMLElement;
    const selectorTarget = target.closest<HTMLElement>(
      "[data-ol-selector-for]"
    );
    const selector = selectorTarget?.dataset?.olSelectorFor;
    if (!selector) return;
    const children = document.querySelector<HTMLElement>(
      `[data-ol-children-for='${selector}']`
    );
    const icon = document.querySelector<SVGElement>(
      `[data-ol-selector-for='${selector}'] .ol-folder-icon`
    );
    if (!children || !icon) return;
    const open = !children.classList.contains("open");
    children.classList.toggle("open", open);
    icon.classList.toggle("open", open);
    map.set(selector, open);
    save();
  };

  root
    .querySelectorAll<HTMLElement>(".ol-folder-icon, .ol-folder-button")
    .forEach((el) => {
      el.addEventListener("click", toggle);
      registerCleanup?.(() => el.removeEventListener("click", toggle));
    });

  // Restore saved state
  for (const [folder, open] of map.entries()) {
    setFolder(folder, open);
  }
}

async function hydrateOverlayExplorer() {
  const openButton = document.getElementById(
    "overlay-explorer-button"
  ) as HTMLButtonElement | null;
  const container = document.getElementById("overlay-explorer-container");
  const list = document.getElementById("overlay-explorer-ul");
  if (!openButton || !container || !list) return;

  const opts: ExplorerOptions = {
    folderClickBehavior:
      (openButton.dataset.behavior as ExplorerClickBehavior) || "mixed",
    folderDefaultState:
      (openButton.dataset.collapsed as ExplorerDefaultState) || "collapsed",
    useSavedState: openButton.dataset.olsavestate === "true",
  };

  const currentSlug = ((window.location.pathname || "/").replace(/^\//, "") ||
    "index") as FullSlug;

  // Rebuild list from content index if available
  if (typeof fetchData !== "undefined") {
    try {
      const data = await fetchData;
      const trie = FileTrieNode.fromEntries([...Object.entries(data)] as [
        string,
        ContentEntry
      ][]);
      list.replaceChildren();
      const sortedChildren = sortChildren(trie.children as ContentNode[]);
      for (const child of sortedChildren) {
        list.appendChild(buildNode(child as ContentNode, currentSlug, opts));
      }
    } catch (err) {
      console.warn("Overlay explorer: failed to load content index", err);
    }
  }

  attachFolderToggles(container, opts.useSavedState);

  if (openButton.dataset.overlayReady !== "true") {
    const show = () => {
      container.classList.add("active");
      container.setAttribute("aria-hidden", "false");
      document.documentElement.classList.add("mobile-no-scroll");
    };
    const hide = () => {
      container.classList.remove("active");
      container.setAttribute("aria-hidden", "true");
      document.documentElement.classList.remove("mobile-no-scroll");
    };

    openButton.addEventListener("click", show);
    registerCleanup?.(() => openButton.removeEventListener("click", show));

    registerEscapeHandler(container, hide);
    openButton.dataset.overlayReady = "true";
  }
}

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      hydrateOverlayExplorer();
    },
    {
      once: true,
    }
  );
} else {
  hydrateOverlayExplorer();
}

document.addEventListener("nav", () => {
  hydrateOverlayExplorer();
});
