const parser = new DOMParser();
const previewCache = new Map<string, string>();

let activePopover: HTMLElement | null = null;
let closeTimer: number | undefined;

function sameOrigin(url: URL): boolean {
  return url.origin === window.location.origin;
}

function previewKey(url: URL): string {
  const key = new URL(url);
  key.hash = "";
  key.search = "";
  return key.toString();
}

function rebaseAttribute(el: Element, attr: "href" | "src", base: URL): void {
  const value = el.getAttribute(attr);
  if (!value || value.startsWith("#")) return;

  const rebased = new URL(value, base);
  if (!sameOrigin(rebased)) return;

  el.setAttribute(attr, `${rebased.pathname}${rebased.search}${rebased.hash}`);
}

function normalizeRelativeUrls(root: Element | Document, base: URL): void {
  root.querySelectorAll("[href]").forEach((el) => rebaseAttribute(el, "href", base));
  root.querySelectorAll("[src]").forEach((el) => rebaseAttribute(el, "src", base));
}

function positionPopover(popover: HTMLElement, link: HTMLAnchorElement): void {
  const linkRect = link.getBoundingClientRect();
  const popoverRect = popover.getBoundingClientRect();
  const margin = 12;

  let left = linkRect.left;
  let top = linkRect.bottom + margin;

  if (left + popoverRect.width + margin > window.innerWidth) {
    left = window.innerWidth - popoverRect.width - margin;
  }

  if (top + popoverRect.height + margin > window.innerHeight) {
    top = linkRect.top - popoverRect.height - margin;
  }

  popover.style.left = `${Math.max(margin, left)}px`;
  popover.style.top = `${Math.max(margin, top)}px`;
}

function clearCloseTimer(): void {
  if (closeTimer !== undefined) {
    window.clearTimeout(closeTimer);
    closeTimer = undefined;
  }
}

function scheduleClose(): void {
  clearCloseTimer();
  closeTimer = window.setTimeout(() => {
    activePopover?.remove();
    activePopover = null;
  }, 140);
}

function scrollToHash(popoverInner: HTMLElement, hash: string): void {
  if (!hash) return;

  const id = decodeURIComponent(hash.slice(1));
  const target =
    Array.from(popoverInner.querySelectorAll<HTMLElement>("[id]")).find(
      (el) => el.id === id
    ) ?? null;

  if (target) {
    popoverInner.scrollTop = Math.max(0, target.offsetTop - 12);
  }
}

async function fetchPreviewDocument(url: URL): Promise<Document | null> {
  const key = previewKey(url);
  let html = previewCache.get(key);

  if (!html) {
    const response = await fetch(key, { credentials: "same-origin" }).catch(() => null);
    if (!response?.ok) return null;

    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.toLowerCase().includes("text/html")) return null;

    html = await response.text();
    previewCache.set(key, html);
  }

  const documentPreview = parser.parseFromString(html, "text/html");
  normalizeRelativeUrls(documentPreview, url);
  return documentPreview;
}

async function showPopover(link: HTMLAnchorElement): Promise<void> {
  if (link.dataset.noPopover === "true") return;

  const targetUrl = new URL(link.href, window.location.href);
  if (!sameOrigin(targetUrl)) return;

  const previewDocument = await fetchPreviewDocument(targetUrl);
  if (!previewDocument) return;

  const hints = previewDocument.querySelectorAll<HTMLElement>(".popover-hint");
  if (hints.length === 0) return;
  if (!link.matches(":hover")) return;

  activePopover?.remove();

  const popover = document.createElement("div");
  popover.className = "popover active-popover";
  popover.setAttribute("role", "dialog");

  const inner = document.createElement("div");
  inner.className = "popover-inner";
  popover.append(inner);

  hints.forEach((hint) => inner.append(document.importNode(hint, true)));

  popover.addEventListener("mouseenter", clearCloseTimer);
  popover.addEventListener("mouseleave", scheduleClose);
  document.body.append(popover);
  activePopover = popover;

  positionPopover(popover, link);
  scrollToHash(inner, targetUrl.hash);
}

function mountPopoverLink(link: HTMLAnchorElement): void {
  if (link.dataset.popoverMounted === "true") return;

  link.addEventListener("mouseenter", () => {
    clearCloseTimer();
    void showPopover(link);
  });
  link.addEventListener("mouseleave", scheduleClose);
  link.dataset.popoverMounted = "true";
}

export function initPopovers(root: ParentNode = document): void {
  root
    .querySelectorAll<HTMLAnchorElement>("a.internal[href]")
    .forEach((link) => mountPopoverLink(link));
}
