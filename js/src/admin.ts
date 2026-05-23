function nullableParentId(value: FormDataEntryValue | null): number | null {
  if (typeof value !== "string" || value.trim() === "") return null;

  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

async function sendJson(url: string, method: string, body: unknown) {
  const response = await fetch(url, {
    method,
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(body),
  });

  if (response.status === 401) {
    window.location.href = "/admin";
    return undefined;
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

function bootAdminForms() {
  setCurrentParents();

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
        window.location.reload();
      } catch (error) {
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
          window.location.reload();
        } catch (error) {
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
          window.location.reload();
        } catch (error) {
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
          credentials: "same-origin",
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

        window.location.reload();
      });
    });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", bootAdminForms);
} else {
  bootAdminForms();
}
