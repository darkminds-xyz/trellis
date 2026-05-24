import { addCleanup } from "../utils/cleanup";

const mountedAttribute = "data-toc-mounted";
const headingSelector =
  '.page-content[data-renderer="rushdown"] :is(h1[id], h2[id], h3[id], h4[id], h5[id], h6[id])';

const observer = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    const slug = entry.target.id;
    const tocEntryElement = document.querySelector(
      `a[data-for="${CSS.escape(slug)}"]`,
    );
    const windowHeight = entry.rootBounds?.height;
    if (windowHeight && tocEntryElement) {
      if (entry.boundingClientRect.y < windowHeight) {
        tocEntryElement.classList.add("in-view");
      } else {
        tocEntryElement.classList.remove("in-view");
      }
    }
  }
});

function toggleToc(this: HTMLElement) {
  this.classList.toggle("collapsed");
  const content = this.nextElementSibling as HTMLElement | undefined;
  if (!content) return;
  content.classList.toggle("collapsed");
  content.style.maxHeight =
    content.style.maxHeight === "0px" ? `${content.scrollHeight}px` : "0px";
}

function setupToc(root: ParentNode = document) {
  const toc = root.querySelector<HTMLElement>("#toc");
  if (!toc) return;

  const collapsed = toc.classList.contains("collapsed");
  const content = toc.nextElementSibling as HTMLElement | undefined;
  if (!content) return;

  content.style.maxHeight = collapsed ? "0px" : `${content.scrollHeight}px`;

  if (toc.getAttribute(mountedAttribute) === "true") return;
  toc.addEventListener("click", toggleToc);
  addCleanup(() => toc.removeEventListener("click", toggleToc));
  toc.setAttribute(mountedAttribute, "true");
}

function observeHeadings(root: ParentNode = document) {
  observer.disconnect();
  root
    .querySelectorAll<HTMLElement>(headingSelector)
    .forEach((header) => observer.observe(header));
}

export function initToc(root: ParentNode = document) {
  setupToc(root);
  observeHeadings(root);
}

const onResize = () => setupToc();

window.addEventListener("resize", onResize);
addCleanup(() => window.removeEventListener("resize", onResize));
