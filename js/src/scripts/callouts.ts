function toggleCallout(this: HTMLElement, evt: MouseEvent) {
  evt.stopPropagation();
  const outerBlock = this.parentElement as HTMLElement | null;
  if (!outerBlock) return;

  outerBlock.classList.toggle("is-collapsed");
  const content = outerBlock.getElementsByClassName("callout-content")[0] as HTMLElement | undefined;
  if (!content) return;

  const collapsed = outerBlock.classList.contains("is-collapsed");
  content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";

  const foldIcon = outerBlock.getElementsByClassName("fold-callout-icon")[0] as
    | HTMLElement
    | undefined;
  foldIcon?.setAttribute("aria-expanded", collapsed ? "false" : "true");
}

export function initCallouts(root: ParentNode = document) {
  root.querySelectorAll<HTMLElement>(".callout").forEach((callout) => {
    const title = callout.getElementsByClassName("callout-title")[0] as HTMLElement | undefined;
    const content = callout.getElementsByClassName("callout-content")[0] as HTMLElement | undefined;
    if (!title || !content) {
      return;
    }

    if (title.dataset.calloutMounted) {
      return;
    }

    title.addEventListener("click", toggleCallout);
    const addCleanup =
      typeof window !== "undefined" && typeof (window as any).addCleanup === "function"
        ? (window as any).addCleanup
        : null;
    addCleanup?.(() => title.removeEventListener("click", toggleCallout));

    const collapsed = callout.classList.contains("is-collapsed");
    content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";

    const foldIcon = callout.getElementsByClassName("fold-callout-icon")[0] as
      | HTMLElement
      | undefined;
    foldIcon?.setAttribute("aria-expanded", collapsed ? "false" : "true");
    title.dataset.calloutMounted = "true";
  });
}
