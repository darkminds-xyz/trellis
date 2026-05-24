class RedirectedForAuth extends Error {}

function nullableParentId(value: FormDataEntryValue | null): number | null {
  if (typeof value !== "string" || value.trim() === "") return null;

  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

async function sendJson(url: string, method: string, body: unknown) {
  const response = await fetch(url, {
    method,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(body),
  });

  if (response.status === 401) {
    window.location.assign("/admin");
    throw new RedirectedForAuth();
  }

  const payload = await response.json().catch(() => undefined);
  if (!response.ok) {
    throw new Error(payload?.message ?? "Unable to save changes");
  }

  return payload;
}

function setCurrentParents() {
  document
    .querySelectorAll<HTMLFormElement>("[data-current-parent]")
    .forEach((form) => {
      const select = form.querySelector<HTMLSelectElement>("select[name='parent_id']");
      if (!select) return;

      const currentParent = form.dataset.currentParent ?? "";
      select.value = currentParent;

      const documentId = form.dataset.documentId;
      if (documentId) {
        select
          .querySelector<HTMLOptionElement>(`option[value='${documentId}']`)
          ?.setAttribute("disabled", "disabled");
      }
    });
}

const collapsedFolders = new Set<string>();
let activeFilter = "all";
let activeFolder: string | null = null;
let activeSearch = "";

function rowParentId(row: HTMLElement): string | null {
  const parent = row.dataset.adminParentId;
  return parent && parent !== "null" ? parent : null;
}

function descendantsOf(folderId: string): HTMLElement[] {
  const rows = Array.from(document.querySelectorAll<HTMLElement>("[data-admin-row]"));
  const byParent = new Map<string, HTMLElement[]>();
  rows.forEach((row) => {
    const parent = rowParentId(row);
    if (!parent) return;
    const list = byParent.get(parent) ?? [];
    list.push(row);
    byParent.set(parent, list);
  });

  const descendants: HTMLElement[] = [];
  const visit = (id: string) => {
    for (const child of byParent.get(id) ?? []) {
      descendants.push(child);
      const childId = child.dataset.adminRowId;
      if (childId) visit(childId);
    }
  };
  visit(folderId);
  return descendants;
}

function ancestorCollapsed(row: HTMLElement): boolean {
  let parent = rowParentId(row);
  while (parent) {
    if (collapsedFolders.has(parent)) return true;
    const parentRow = document.querySelector<HTMLElement>(
      `[data-admin-row][data-admin-row-id='${parent}']`
    );
    parent = parentRow ? rowParentId(parentRow) : null;
  }
  return false;
}

function ancestorCollapsedByNode(node: HTMLElement): boolean {
  let parent = rowParentId(node);
  while (parent) {
    if (collapsedFolders.has(parent)) return true;
    const parentNode =
      document.querySelector<HTMLElement>(`[data-admin-rail-node][data-admin-row-id='${parent}']`) ??
      document.querySelector<HTMLElement>(`[data-admin-row][data-admin-row-id='${parent}']`);
    parent = parentNode ? rowParentId(parentNode) : null;
  }
  return false;
}

function matchesActiveFilter(row: HTMLElement): boolean {
  if (activeSearch && !row.textContent?.toLowerCase().includes(activeSearch)) {
    return false;
  }

  if (activeFolder) {
    if (row.dataset.adminRowId === activeFolder) return true;
    return descendantsOf(activeFolder).includes(row);
  }

  if (activeFilter === "draft") {
    return row.dataset.adminKind === "note" && row.dataset.adminDraft === "true";
  }
  if (activeFilter === "published") {
    return (
      row.dataset.adminKind === "note" &&
      row.dataset.adminDraft !== "true" &&
      row.dataset.adminHidden !== "true"
    );
  }
  if (activeFilter === "hidden") {
    return row.dataset.adminHidden === "true";
  }

  return true;
}

function syncFolderUi(): void {
  document.querySelectorAll<HTMLElement>("[data-admin-folder-toggle]").forEach((button) => {
    const id = button.dataset.adminFolderId;
    const open = !id || !collapsedFolders.has(id);
    button.classList.toggle("collapsed", !open);
    button.setAttribute("aria-expanded", String(open));
  });

  document.querySelectorAll<HTMLElement>("[data-admin-row]").forEach((row) => {
    const visible = matchesActiveFilter(row) && !ancestorCollapsed(row);
    row.hidden = !visible;
  });

  document.querySelectorAll<HTMLElement>("[data-admin-rail-node]").forEach((node) => {
    const id = node.dataset.adminRowId;
    node.hidden = ancestorCollapsedByNode(node);
    node.classList.toggle("active", Boolean(id && id === activeFolder));
  });
}

function bootAdminNavigation() {
  document.querySelectorAll<HTMLElement>("[data-admin-folder-toggle]").forEach((button) => {
    if (button.dataset.adminMounted === "true") return;
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const id = button.dataset.adminFolderId;
      if (!id) return;
      if (collapsedFolders.has(id)) {
        collapsedFolders.delete(id);
      } else {
        collapsedFolders.add(id);
      }
      syncFolderUi();
    });
    button.dataset.adminMounted = "true";
  });

  document.querySelectorAll<HTMLElement>("[data-admin-folder-filter]").forEach((button) => {
    if (button.dataset.adminMounted === "true") return;
    button.addEventListener("click", (event) => {
      event.preventDefault();
      activeFolder = button.dataset.adminFolderFilter ?? null;
      activeFilter = "all";
      document
        .querySelectorAll<HTMLElement>("[data-admin-filter]")
        .forEach((el) => el.classList.remove("active"));
      syncFolderUi();
    });
    button.dataset.adminMounted = "true";
  });

  document.querySelectorAll<HTMLElement>("[data-admin-filter]").forEach((button) => {
    if (button.dataset.adminMounted === "true") return;
    button.addEventListener("click", () => {
      activeFilter = button.dataset.adminFilter ?? "all";
      activeFolder = null;
      document
        .querySelectorAll<HTMLElement>("[data-admin-filter]")
        .forEach((el) => el.classList.toggle("active", el === button));
      syncFolderUi();
    });
    button.dataset.adminMounted = "true";
  });

  document.querySelector<HTMLInputElement>(".admin-list-tools input[type='search']")?.addEventListener("input", (event) => {
    activeSearch = (event.currentTarget as HTMLInputElement).value.trim().toLowerCase();
    syncFolderUi();
  });

  syncFolderUi();
}

function bootFolderRename() {
  document.querySelectorAll<HTMLFormElement>("[data-admin-update-folder]").forEach((form) => {
    const label = form.querySelector<HTMLButtonElement>("[data-admin-folder-name-text]");
    const input = form.querySelector<HTMLInputElement>("[data-admin-folder-name-input]");
    if (!label || !input || input.dataset.renameMounted === "true") return;

    let original = input.value;
    const showInput = () => {
      original = input.value;
      label.hidden = true;
      input.hidden = false;
      input.focus();
      input.select();
    };
    const hideInput = () => {
      input.hidden = true;
      label.hidden = false;
    };
    const save = async () => {
      const next = input.value.trim();
      if (!next) {
        input.value = original;
        hideInput();
        return;
      }
      if (next === original) {
        hideInput();
        return;
      }

      const documentId = form.dataset.documentId;
      const data = new FormData(form);
      if (!documentId) return;

      try {
        await sendJson(`/api/admin/folders/${documentId}`, "PATCH", {
          name: next,
          parent_id: nullableParentId(data.get("parent_id")),
          hidden: data.get("hidden") === "on",
        });
        label.textContent = next;
        original = next;
        hideInput();
      } catch (error) {
        if (error instanceof RedirectedForAuth) return;
        window.alert(error instanceof Error ? error.message : "Unable to rename folder.");
        input.value = original;
        hideInput();
      }
    };

    label.addEventListener("dblclick", showInput);
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        void save();
      }
      if (event.key === "Escape") {
        input.value = original;
        hideInput();
      }
    });
    input.addEventListener("blur", () => void save());
    input.dataset.renameMounted = "true";
  });
}

function bootAdminForms() {
  setCurrentParents();
  bootAdminNavigation();
  bootFolderRename();

  document
    .querySelector<HTMLFormElement>("[data-admin-create-folder]")
    ?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const form = event.currentTarget as HTMLFormElement;
      const data = new FormData(form);

      try {
        await sendJson("/api/admin/folders", "POST", {
          name: data.get("name"),
          parent_id: nullableParentId(data.get("parent_id")),
          hidden: data.get("hidden") === "on",
        });
        window.location.assign("/admin/list");
      } catch (error) {
        if (error instanceof RedirectedForAuth) return;
        window.alert(error instanceof Error ? error.message : "Unable to create folder.");
      }
    });

  document
    .querySelector<HTMLFormElement>("[data-admin-create-note]")
    ?.addEventListener("submit", async (event) => {
      event.preventDefault();
      const form = event.currentTarget as HTMLFormElement;
      const data = new FormData(form);
      const name = String(data.get("name") ?? "note.md");
      const title = name.replace(/\.md$/i, "").replace(/[-_]+/g, " ");
      try {
        const payload = await sendJson("/api/admin/notes", "POST", {
          name,
          parent_id: nullableParentId(data.get("parent_id")),
          draft: data.get("draft") === "on",
          markdown: `# ${title}\n\n`,
        });

        if (payload?.id) {
          window.location.href = `/admin/edit/${payload.id}`;
        }
      } catch (error) {
        if (error instanceof RedirectedForAuth) return;
        window.alert(error instanceof Error ? error.message : "Unable to create note.");
      }
    });

  document
    .querySelectorAll<HTMLFormElement>("[data-admin-update-folder]")
    .forEach((form) => {
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        const documentId = form.dataset.documentId;
        if (!documentId) return;

        const data = new FormData(form);
        try {
          await sendJson(`/api/admin/folders/${documentId}`, "PATCH", {
            name: data.get("name"),
            parent_id: nullableParentId(data.get("parent_id")),
            hidden: data.get("hidden") === "on",
          });
          window.location.assign("/admin/list");
        } catch (error) {
          if (error instanceof RedirectedForAuth) return;
          window.alert(error instanceof Error ? error.message : "Unable to save folder.");
        }
      });
    });

  document
    .querySelectorAll<HTMLFormElement>("[data-admin-update-note]")
    .forEach((form) => {
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        const documentId = form.dataset.documentId;
        if (!documentId) return;

        const data = new FormData(form);
        try {
          await sendJson(`/api/admin/notes/${documentId}`, "PATCH", {
            name: data.get("name"),
            parent_id: nullableParentId(data.get("parent_id")),
            draft: data.get("draft") === "on",
          });
          window.location.assign("/admin/list");
        } catch (error) {
          if (error instanceof RedirectedForAuth) return;
          window.alert(error instanceof Error ? error.message : "Unable to save note.");
        }
      });
    });

  document
    .querySelectorAll<HTMLButtonElement>("[data-admin-delete-document]")
    .forEach((button) => {
      button.addEventListener("click", async () => {
        const documentId = button.dataset.documentId;
        if (!documentId) return;

        const kind = button.dataset.documentKind ?? "document";
        if (!window.confirm(`Delete this ${kind}?`)) return;

        const response = await fetch(`/api/admin/documents/${documentId}`, {
          method: "DELETE",
          credentials: "include",
          headers: { Accept: "application/json" },
        });

        if (response.status === 401) {
          window.location.href = "/admin";
          return;
        }

        if (response.status === 409) {
          const payload = await response.json().catch(() => undefined);
          window.alert(payload?.message ?? "Folder contains notes.");
          return;
        }

        if (!response.ok) {
          window.alert("Unable to delete document.");
          return;
        }

        window.location.assign("/admin/list");
      });
    });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootAdminForms);
} else {
  bootAdminForms();
}
