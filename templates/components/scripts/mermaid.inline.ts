import { registerEscapeHandler, removeAllChildren } from "./util";

// Trellis renders without the SPA router in some modes, so addCleanup may be
// undefined. Provide a no-op fallback so event cleanup calls don't explode.
const registerCleanup: (fn: () => void) => void =
  typeof window !== "undefined" && typeof window.addCleanup === "function"
    ? window.addCleanup
    : () => {};

class DiagramPanZoom {
  isDragging = false;
  startPan: { x: number; y: number } = { x: 0, y: 0 };
  currentPan: { x: number; y: number } = { x: 0, y: 0 };
  scale = 1;
  readonly MIN_SCALE = 0.5;
  readonly MAX_SCALE = 3;

  cleanups: Array<() => void> = [];
  private container: HTMLElement;
  private content: HTMLElement;

  constructor(container: HTMLElement, content: HTMLElement) {
    this.container = container;
    this.content = content;
    this.setupEventListeners();
    this.setupNavigationControls();
    this.resetTransform();
  }

  setupEventListeners(): void {
    // Mouse drag events
    const mouseDownHandler = this.onMouseDown.bind(this);
    const mouseMoveHandler = this.onMouseMove.bind(this);
    const mouseUpHandler = this.onMouseUp.bind(this);

    // Touch drag events
    const touchStartHandler = this.onTouchStart.bind(this);
    const touchMoveHandler = this.onTouchMove.bind(this);
    const touchEndHandler = this.onTouchEnd.bind(this);

    const resizeHandler = this.resetTransform.bind(this);

    this.container.addEventListener("mousedown", mouseDownHandler);
    document.addEventListener("mousemove", mouseMoveHandler);
    document.addEventListener("mouseup", mouseUpHandler);

    this.container.addEventListener("touchstart", touchStartHandler, {
      passive: false,
    });
    document.addEventListener("touchmove", touchMoveHandler, {
      passive: false,
    });
    document.addEventListener("touchend", touchEndHandler);

    window.addEventListener("resize", resizeHandler);

    this.cleanups.push(
      () => this.container.removeEventListener("mousedown", mouseDownHandler),
      () => document.removeEventListener("mousemove", mouseMoveHandler),
      () => document.removeEventListener("mouseup", mouseUpHandler),
      () => this.container.removeEventListener("touchstart", touchStartHandler),
      () => document.removeEventListener("touchmove", touchMoveHandler),
      () => document.removeEventListener("touchend", touchEndHandler),
      () => window.removeEventListener("resize", resizeHandler)
    );
  }

  cleanup(): void {
    for (const cleanup of this.cleanups) {
      cleanup();
    }
  }

  setupNavigationControls(): void {
    const controls = document.createElement("div");
    controls.className = "mermaid-controls";

    // Zoom controls
    const zoomIn = this.createButton("+", () => this.zoom(0.1));
    const zoomOut = this.createButton("-", () => this.zoom(-0.1));
    const resetBtn = this.createButton("Reset", () => this.resetTransform());

    controls.appendChild(zoomOut);
    controls.appendChild(resetBtn);
    controls.appendChild(zoomIn);

    this.container.appendChild(controls);
  }

  createButton(text: string, onClick: () => void): HTMLButtonElement {
    const button = document.createElement("button");
    button.textContent = text;
    button.className = "mermaid-control-button";
    button.addEventListener("click", onClick);
    registerCleanup(() => button.removeEventListener("click", onClick));
    return button;
  }

  onMouseDown(e: MouseEvent): void {
    if (e.button !== 0) return; // Only handle left click
    this.isDragging = true;
    this.startPan = {
      x: e.clientX - this.currentPan.x,
      y: e.clientY - this.currentPan.y,
    };
    this.container.style.cursor = "grabbing";
  }

  onMouseMove(e: MouseEvent): void {
    if (!this.isDragging) return;
    e.preventDefault();

    this.currentPan = {
      x: e.clientX - this.startPan.x,
      y: e.clientY - this.startPan.y,
    };

    this.updateTransform();
  }

  onMouseUp(): void {
    this.isDragging = false;
    this.container.style.cursor = "grab";
  }

  onTouchStart(e: TouchEvent): void {
    if (e.touches.length !== 1) return;
    this.isDragging = true;
    const touch = e.touches[0];
    this.startPan = {
      x: touch.clientX - this.currentPan.x,
      y: touch.clientY - this.currentPan.y,
    };
  }

  onTouchMove(e: TouchEvent): void {
    if (!this.isDragging || e.touches.length !== 1) return;
    e.preventDefault(); // Prevent scrolling

    const touch = e.touches[0];
    this.currentPan = {
      x: touch.clientX - this.startPan.x,
      y: touch.clientY - this.startPan.y,
    };

    this.updateTransform();
  }

  onTouchEnd(): void {
    this.isDragging = false;
  }

  zoom(delta: number): void {
    const newScale = Math.min(
      Math.max(this.scale + delta, this.MIN_SCALE),
      this.MAX_SCALE
    );

    // Zoom around center
    const rect = this.content.getBoundingClientRect();
    const centerX = rect.width / 2;
    const centerY = rect.height / 2;

    const scaleDiff = newScale - this.scale;
    this.currentPan.x -= centerX * scaleDiff;
    this.currentPan.y -= centerY * scaleDiff;

    this.scale = newScale;
    this.updateTransform();
  }

  updateTransform(): void {
    this.content.style.transform = `translate(${this.currentPan.x}px, ${this.currentPan.y}px) scale(${this.scale})`;
  }

  resetTransform(): void {
    const svg = this.content.querySelector("svg");
    if (!svg) return;
    const rect = svg.getBoundingClientRect();
    const width = rect.width / this.scale;
    const height = rect.height / this.scale;

    this.scale = 1;
    this.currentPan = {
      x: (this.container.clientWidth - width) / 2,
      y: (this.container.clientHeight - height) / 2,
    };
    this.updateTransform();
  }
}

const cssVars: readonly string[] = [
  "--secondary",
  "--tertiary",
  "--gray",
  "--light",
  "--lightgray",
  "--highlight",
  "--dark",
  "--darkgray",
  "--codeFont",
];

let mermaidImport: { default: any } | undefined;

async function hydrateMermaid(): Promise<void> {
  const center = document.querySelector<HTMLElement>(".center");
  if (!center) return;

  const nodes = center.querySelectorAll<HTMLElement>("code.mermaid");
  if (nodes.length === 0) return;

  mermaidImport ||= await import(
    // @ts-ignore
    "https://cdnjs.cloudflare.com/ajax/libs/mermaid/11.4.0/mermaid.esm.min.mjs"
  );
  if (!mermaidImport) return;
  const mermaid = mermaidImport.default;

  const textMapping = new WeakMap<HTMLElement, string>();
  for (const node of nodes) {
    textMapping.set(node, node.innerText);
  }

  async function renderMermaid(): Promise<void> {
    // de-init any other diagrams
    for (const node of nodes) {
      node.removeAttribute("data-processed");
      const oldText = textMapping.get(node);
      if (oldText) {
        node.innerHTML = oldText;
      }
    }

    const computedStyleMap = cssVars.reduce<Record<string, string>>(
      (acc, key) => {
        acc[key] = window
          .getComputedStyle(document.documentElement)
          .getPropertyValue(key);
        return acc;
      },
      {}
    );

    const darkMode =
      document.documentElement.getAttribute("saved-theme") === "dark";
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "loose",
      theme: darkMode ? "dark" : "base",
      themeVariables: {
        fontFamily: computedStyleMap["--codeFont"],
        primaryColor: computedStyleMap["--light"],
        primaryTextColor: computedStyleMap["--darkgray"],
        primaryBorderColor: computedStyleMap["--tertiary"],
        lineColor: computedStyleMap["--darkgray"],
        secondaryColor: computedStyleMap["--secondary"],
        tertiaryColor: computedStyleMap["--tertiary"],
        clusterBkg: computedStyleMap["--light"],
        edgeLabelBackground: computedStyleMap["--highlight"],
      },
    });

    await mermaid.run({ nodes });
  }

  await renderMermaid();
  document.addEventListener("themechange", renderMermaid);
  registerCleanup(() =>
    document.removeEventListener("themechange", renderMermaid)
  );

  for (let i = 0; i < nodes.length; i++) {
    const codeBlock = nodes[i];
    const pre = codeBlock.parentElement as HTMLElement | null;
    if (!pre) continue;

    const clipboardBtn =
      pre.querySelector<HTMLButtonElement>(".clipboard-button");
    const expandBtn =
      pre.querySelector<HTMLButtonElement>(".expand-button") ||
      createExpandButton(pre);

    const clipboardStyle = clipboardBtn
      ? window.getComputedStyle(clipboardBtn)
      : null;
    const clipboardWidth = clipboardBtn
      ? clipboardBtn.offsetWidth +
        parseFloat(clipboardStyle?.marginLeft ?? "0") +
        parseFloat(clipboardStyle?.marginRight ?? "0")
      : 0;

    // Set expand button position
    expandBtn.style.right = `calc(${clipboardWidth}px + 0.3rem)`;
    pre.prepend(expandBtn);

    // query popup container
    const popupContainer =
      pre.querySelector<HTMLDivElement>("#mermaid-container");
    if (!popupContainer) continue;
    const popupContainerEl = popupContainer; // narrowed non-null

    let panZoom: DiagramPanZoom | null = null;
    function showMermaid() {
      const container =
        popupContainerEl.querySelector<HTMLDivElement>("#mermaid-space");
      const content =
        popupContainerEl.querySelector<HTMLDivElement>(".mermaid-content");
      if (!container || !content) return;
      removeAllChildren(content);

      // Clone the mermaid content
      const mermaidContent = codeBlock
        .querySelector("svg")
        ?.cloneNode(true) as SVGElement | null;
      if (!mermaidContent) return;
      content.appendChild(mermaidContent);

      // Show container
      popupContainerEl.classList.add("active");
      container.style.cursor = "grab";

      // Initialize pan-zoom after showing the popup
      panZoom = new DiagramPanZoom(container, content);
    }

    function hideMermaid() {
      popupContainerEl.classList.remove("active");
      panZoom?.cleanup();
      panZoom = null;
    }

    expandBtn.addEventListener("click", showMermaid);
    registerEscapeHandler(popupContainerEl, hideMermaid);

    registerCleanup(() => {
      panZoom?.cleanup();
      expandBtn.removeEventListener("click", showMermaid);
    });
  }
}

function createExpandButton(pre: HTMLElement): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.className = "expand-button";
  btn.setAttribute("aria-label", "Expand mermaid diagram");
  btn.setAttribute("data-view-component", "true");
  btn.innerHTML =
    '<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M3.72 3.72a.75.75 0 011.06 1.06L2.56 7h10.88l-2.22-2.22a.75.75 0 011.06-1.06l3.5 3.5a.75.75 0 010 1.06l-3.5 3.5a.75.75 0 11-1.06-1.06l2.22-2.22H2.56l2.22 2.22a.75.75 0 11-1.06 1.06l-3.5-3.5a.75.75 0 010-1.06l3.5-3.5z"></path></svg>';
  return btn;
}

// Initial hydration for SSR/non-SPA load.
if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      hydrateMermaid();
    },
    { once: true }
  );
} else {
  hydrateMermaid();
}

// Hydrate on SPA nav events.
document.addEventListener("nav", () => {
  hydrateMermaid();
});
