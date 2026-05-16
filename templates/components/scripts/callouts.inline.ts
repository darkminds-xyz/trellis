function toggleCallout(this: HTMLElement, evt: MouseEvent) {
  evt.stopPropagation();
  const outerBlock = this.parentElement as HTMLElement | null;
  if (!outerBlock) return;

  outerBlock.classList.toggle("is-collapsed");
  const content = outerBlock.getElementsByClassName("callout-content")[0] as
    | HTMLElement
    | undefined;
  if (!content) return;
  const collapsed = outerBlock.classList.contains("is-collapsed");
  content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";
}

function setupCallout(): void {
  const collapsible = document.getElementsByClassName("callout");
  for (const div of collapsible) {
    const title = div.getElementsByClassName("callout-title")[0] as
      | HTMLElement
      | undefined;
    const content = div.getElementsByClassName("callout-content")[0] as
      | HTMLElement
      | undefined;
    if (!title || !content) continue;

    title.addEventListener("click", toggleCallout);
    window.addCleanup?.(() =>
      title.removeEventListener("click", toggleCallout)
    );

    const collapsed = div.classList.contains("is-collapsed");
    content.style.gridTemplateRows = collapsed ? "0fr" : "1fr";
  }
}
window.onload = () => {
  setupCallout();
};
