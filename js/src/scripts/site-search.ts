type SearchEntry = {
  slug?: string;
  title?: string;
  filePath?: string;
  content?: string;
  tags?: string[];
};

type SearchResult = {
  entry: SearchEntry;
  score: number;
  excerpt: string;
};

declare global {
  interface Window {
    fetchData?: Promise<Record<string, SearchEntry> | undefined>;
  }
}

const maxResults = 12;

function normalize(value: string | undefined) {
  return (value ?? "").toLocaleLowerCase();
}

function resultHref(entry: SearchEntry) {
  if (!entry.slug || entry.slug === "index") {
    return "/";
  }

  return `/${entry.slug.replace(/^\/+/, "")}`;
}

function excerptFor(content: string | undefined, query: string) {
  const text = (content ?? "").replace(/\s+/g, " ").trim();
  if (!text) {
    return "";
  }

  const index = normalize(text).indexOf(normalize(query));
  if (index === -1) {
    return text.slice(0, 180);
  }

  const start = Math.max(0, index - 70);
  const end = Math.min(text.length, index + query.length + 110);
  const prefix = start > 0 ? "... " : "";
  const suffix = end < text.length ? " ..." : "";
  return `${prefix}${text.slice(start, end)}${suffix}`;
}

function scoreEntry(entry: SearchEntry, query: string): SearchResult | undefined {
  const title = normalize(entry.title);
  const path = normalize(entry.filePath);
  const tags = normalize((entry.tags ?? []).join(" "));
  const content = normalize(entry.content);
  const normalizedQuery = normalize(query);

  let score = 0;
  if (title === normalizedQuery) score += 100;
  if (title.startsWith(normalizedQuery)) score += 60;
  if (title.includes(normalizedQuery)) score += 40;
  if (path.includes(normalizedQuery)) score += 20;
  if (tags.includes(normalizedQuery)) score += 18;
  if (content.includes(normalizedQuery)) score += 8;

  if (score === 0) {
    return undefined;
  }

  return {
    entry,
    score,
    excerpt: excerptFor(entry.content, query),
  };
}

function renderResults(
  resultsEl: HTMLOListElement,
  statusEl: HTMLElement,
  results: SearchResult[],
  query: string,
) {
  resultsEl.replaceChildren();

  if (!query.trim()) {
    statusEl.textContent = "Start typing to search every post.";
    return;
  }

  if (results.length === 0) {
    statusEl.textContent = `No posts found for "${query}".`;
    return;
  }

  statusEl.textContent = `${results.length} result${results.length === 1 ? "" : "s"}`;

  for (const { entry, excerpt } of results) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    const title = document.createElement("span");
    const path = document.createElement("span");

    link.href = resultHref(entry);
    title.className = "site-search-result-title";
    title.textContent = entry.title ?? "Untitled";
    path.className = "site-search-result-path";
    path.textContent = entry.filePath ?? entry.slug ?? "";

    link.append(title, path);

    if (excerpt) {
      const preview = document.createElement("p");
      preview.textContent = excerpt;
      link.append(preview);
    }

    item.append(link);
    resultsEl.append(item);
  }
}

async function loadEntries() {
  const data =
    (await window.fetchData?.catch(() => undefined)) ??
    (await fetch("/static/context-index.json")
      .then((response) => response.json() as Promise<Record<string, SearchEntry>>)
      .catch(() => undefined));

  return Object.values(data ?? {});
}

export function initSiteSearch() {
  const root = document.querySelector<HTMLElement>("[data-site-search]");
  const openButton = root?.querySelector<HTMLButtonElement>("[data-site-search-open]");
  const modal = root?.querySelector<HTMLElement>("[data-site-search-modal]");
  const closeButton = root?.querySelector<HTMLButtonElement>("[data-site-search-close]");
  const input = root?.querySelector<HTMLInputElement>("[data-site-search-input]");
  const resultsEl = root?.querySelector<HTMLOListElement>("[data-site-search-results]");
  const statusEl = root?.querySelector<HTMLElement>("[data-site-search-status]");

  if (!root || !openButton || !modal || !closeButton || !input || !resultsEl || !statusEl) {
    return;
  }

  let entries: SearchEntry[] = [];

  const close = () => {
    modal.classList.remove("active");
    modal.setAttribute("aria-hidden", "true");
    document.body.classList.remove("site-search-lock");
    openButton.focus();
  };

  const open = async () => {
    modal.classList.add("active");
    modal.setAttribute("aria-hidden", "false");
    document.body.classList.add("site-search-lock");
    window.setTimeout(() => input.focus(), 80);

    if (entries.length === 0) {
      statusEl.textContent = "Loading posts...";
      entries = await loadEntries();
      renderResults(resultsEl, statusEl, [], input.value);
    }
  };

  const update = () => {
    const query = input.value.trim();
    const results = entries
      .map((entry) => scoreEntry(entry, query))
      .filter((result): result is SearchResult => Boolean(result))
      .sort((a, b) => b.score - a.score || (a.entry.title ?? "").localeCompare(b.entry.title ?? ""))
      .slice(0, maxResults);

    renderResults(resultsEl, statusEl, results, query);
  };

  openButton.addEventListener("click", open);
  closeButton.addEventListener("click", close);
  input.addEventListener("input", update);
  modal.addEventListener("click", (event) => {
    if (event.target === modal) {
      close();
    }
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && modal.classList.contains("active")) {
      close();
    }
  });
}
