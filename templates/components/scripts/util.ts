declare global {
  interface Window {
    addCleanup?: (fn: () => void) => void;
  }
}

export function registerEscapeHandler(
  outsideContainer: HTMLElement | null | undefined,
  cb: () => void
) {
  const addCleanup: ((fn: () => void) => void) | null =
    typeof window !== "undefined" && typeof window.addCleanup === "function"
      ? window.addCleanup
      : null;
  if (!outsideContainer) return;
  function click(this: HTMLElement, e: MouseEvent) {
    if (e.target !== this) return;
    e.preventDefault();
    e.stopPropagation();
    cb();
  }

  function esc(e: KeyboardEvent) {
    if (!e.key || !e.key.startsWith("Esc")) return;
    e.preventDefault();
    cb();
  }

  outsideContainer?.addEventListener("click", click);
  addCleanup?.(() => outsideContainer?.removeEventListener("click", click));
  document.addEventListener("keydown", esc);
  addCleanup?.(() => document.removeEventListener("keydown", esc));
}

export function removeAllChildren(node: ParentNode) {
  while (node.firstChild) {
    node.removeChild(node.firstChild);
  }
}

// AliasRedirect emits HTML redirects which also have the link[rel="canonical"]
// containing the URL it's redirecting to.
// Extracting it here with regex is _probably_ faster than parsing the entire HTML
// with a DOMParser effectively twice (here and later in the SPA code), even if
// way less robust - we only care about our own generated redirects after all.
const canonicalRegex = /<link rel="canonical" href="([^"]*)">/;

export async function fetchCanonical(url: string): Promise<Response> {
  const res = await fetch(`${url}`);
  if (!res.headers.get("content-type")?.startsWith("text/html")) {
    return res;
  }

  // reading the body can only be done once, so we need to clone the response
  // to allow the caller to read it if it's was not a redirect
  const text = await res.clone().text();
  const [, redirect] = text.match(canonicalRegex) ?? [];
  return redirect ? fetch(`${new URL(redirect, url)}`) : res;
}
