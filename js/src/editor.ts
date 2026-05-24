import { autocompletion, closeBrackets } from "@codemirror/autocomplete";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language";
import { GFM } from "@lezer/markdown";
import { searchKeymap } from "@codemirror/search";
import { EditorState } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  EditorView,
  highlightSpecialChars,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import "./editor.css";

declare const trellisEditor: string;

function editorTheme() {
  return EditorView.theme({
    "&": {
      color: "var(--dark)",
      backgroundColor: "var(--light)",
      height: "100%",
    },
    ".cm-scroller": {
      fontFamily: "var(--codeFont)",
      lineHeight: "1.55",
    },
    ".cm-content": {
      caretColor: "var(--secondary)",
      padding: "0.75rem 0",
    },
    ".cm-line": {
      padding: "0 0.85rem",
    },
    ".cm-gutters": {
      color: "var(--gray)",
      backgroundColor: "color-mix(in srgb, var(--light) 94%, var(--dark) 6%)",
      borderRight: "1px solid var(--lightgray)",
    },
    ".cm-activeLineGutter": {
      color: "var(--secondary)",
      backgroundColor: "color-mix(in srgb, var(--secondary) 10%, transparent)",
    },
    ".cm-activeLine": {
      backgroundColor: "color-mix(in srgb, var(--secondary) 7%, transparent)",
    },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
      backgroundColor: "color-mix(in srgb, var(--secondary) 25%, transparent)",
    },
    ".cm-cursor": {
      borderLeftColor: "var(--secondary)",
    },
    ".cm-foldGutter span": {
      color: "var(--gray)",
    },
    ".cm-tooltip": {
      color: "var(--dark)",
      backgroundColor: "var(--light)",
      border: "1px solid var(--lightgray)",
    },
  });
}

function editorExtensions() {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    foldGutter(),
    drawSelection(),
    dropCursor(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    autocompletion(),
    highlightActiveLine(),
    keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
    markdown({
      extensions: [GFM],
    }),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    editorTheme(),
    EditorView.lineWrapping,
  ];
}

function currentMarkdown(editor: EditorView, sourceView: HTMLTextAreaElement | null): string {
  return sourceView && !sourceView.hidden ? sourceView.value : editor.state.doc.toString();
}

function syncHiddenSource(value: string, source: HTMLInputElement | null, sourceView: HTMLTextAreaElement | null) {
  if (source) source.value = value;
  if (sourceView && sourceView.hidden) sourceView.value = value;
}

function boot() {
  const el = document.getElementById("trellis-editor");
  if (!el) return;
  const source = document.querySelector<HTMLInputElement>(
    "[data-trellis-editor-source]"
  );
  const sourceView = document.querySelector<HTMLTextAreaElement>(
    "[data-trellis-source-view]"
  );
  const sourceToggle = document.querySelector<HTMLButtonElement>(
    "[data-trellis-source-toggle]"
  );
  const previewToggle = document.querySelector<HTMLButtonElement>(
    "[data-trellis-preview-toggle]"
  );
  const preview = document.querySelector<HTMLElement>("[data-trellis-preview]");
  const form = document.querySelector<HTMLFormElement>(
    "[data-trellis-editor-form]"
  );
  const saveError = document.querySelector<HTMLElement>(
    "[data-trellis-save-error]"
  );
  const submitButton = form?.querySelector<HTMLButtonElement>(
    "button[type='submit']"
  );
  const noteName = document.querySelector<HTMLInputElement>(
    "[data-trellis-note-name]"
  );
  const noteParent = document.querySelector<HTMLSelectElement>(
    "[data-trellis-note-parent]"
  );
  const noteDraft = document.querySelector<HTMLInputElement>(
    "[data-trellis-note-draft]"
  );
  const initialDoc = trellisEditor ?? sourceView?.value ?? source?.value ?? "";
  const editor = new EditorView({
    state: EditorState.create({
      doc: initialDoc,
      extensions: [
        ...editorExtensions(),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            syncHiddenSource(update.state.doc.toString(), source, sourceView);
          }
        }),
      ],
    }),
    parent: el,
  });

  syncHiddenSource(initialDoc, source, sourceView);

  const showEditor = () => {
    if (preview) {
      preview.hidden = true;
    }
    el.hidden = false;
    if (sourceView && sourceToggle?.getAttribute("aria-pressed") === "true") {
      sourceView.hidden = false;
      el.hidden = true;
    }
    previewToggle?.setAttribute("aria-pressed", "false");
    if (previewToggle) previewToggle.textContent = "Preview";
  };

  sourceToggle?.addEventListener("click", () => {
    if (!sourceView || previewToggle?.getAttribute("aria-pressed") === "true") return;

    const showSource = sourceView.hidden;
    if (showSource) {
      const value = editor.state.doc.toString();
      sourceView.value = value;
      if (source) source.value = value;
    } else {
      const value = sourceView.value;
      editor.dispatch({
        changes: {
          from: 0,
          to: editor.state.doc.length,
          insert: value,
        },
      });
      if (source) source.value = value;
    }

    sourceView.hidden = !showSource;
    el.hidden = showSource;
    sourceToggle.setAttribute("aria-pressed", String(showSource));
  });

  sourceView?.addEventListener("input", () => {
    if (source) source.value = sourceView.value;
  });

  previewToggle?.addEventListener("click", async () => {
    if (!preview) return;
    const isPreviewing = previewToggle.getAttribute("aria-pressed") === "true";
    if (isPreviewing) {
      showEditor();
      return;
    }

    const markdown = currentMarkdown(editor, sourceView);
    syncHiddenSource(markdown, source, sourceView);
    previewToggle.disabled = true;
    if (saveError) {
      saveError.hidden = true;
      saveError.textContent = "";
    }

    try {
      const response = await fetch("/api/admin/preview", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify({ doc: markdown }),
      });

      if (response.status === 401) {
        window.location.href = "/admin";
        return;
      }

      const body = await response.json().catch(() => undefined);
      if (!response.ok) {
        throw new Error(body?.message ?? "Unable to render preview");
      }

      preview.innerHTML = body.html ?? "";
      preview.hidden = false;
      el.hidden = true;
      if (sourceView) sourceView.hidden = true;
      previewToggle.setAttribute("aria-pressed", "true");
      previewToggle.textContent = "Edit";
    } catch (error) {
      if (saveError) {
        saveError.hidden = false;
        saveError.textContent =
          error instanceof Error ? error.message : "Unable to render preview";
      }
    } finally {
      previewToggle.disabled = false;
    }
  });

  form?.addEventListener("submit", async (event) => {
    event.preventDefault();

    const saveUrl = form.dataset.saveUrl;
    if (!saveUrl) return;

    const markdown = currentMarkdown(editor, sourceView);
    syncHiddenSource(markdown, source, sourceView);

    if (saveError) {
      saveError.hidden = true;
      saveError.textContent = "";
    }
    if (submitButton) submitButton.disabled = true;

    try {
      const response = await fetch(saveUrl, {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify({
          doc: markdown,
          name: noteName?.value,
          parent_id: noteParent?.value ? Number.parseInt(noteParent.value, 10) : null,
          draft: noteDraft?.checked,
        }),
      });

      if (response.status === 401) {
        window.location.href = saveUrl;
        return;
      }

      const body = await response.json().catch(() => undefined);
      if (!response.ok) {
        throw new Error(body?.message ?? "Unable to save document");
      }

      if (body?.edit_url) {
        window.location.href = body.edit_url;
      }
    } catch (error) {
      if (saveError) {
        saveError.hidden = false;
        saveError.textContent =
          error instanceof Error ? error.message : "Unable to save document";
      }
      if (submitButton) submitButton.disabled = false;
    }
  });

  return editor;
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
