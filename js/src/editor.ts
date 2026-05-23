import { EditorView } from "@codemirror/view";
import { GFM } from "@lezer/markdown";
import {
  prosemarkLightThemeSetup,
  prosemarkBasicSetup,
  prosemarkBaseThemeSetup,
  prosemarkMarkdownSyntaxExtensions,
} from "@prosemark/core";
import { languages } from "@codemirror/language-data";
import { htmlBlockExtension } from "@prosemark/render-html";
import { markdown } from "@codemirror/lang-markdown";
import "./editor.css";

declare const trellisEditor: string;

function boot() {
  const el = document.getElementById("trellis-editor");
  if (!el) return;
  const source = document.querySelector<HTMLInputElement>(
    "[data-trellis-editor-source]",
  );
  const sourceView = document.querySelector<HTMLTextAreaElement>(
    "[data-trellis-source-view]",
  );
  const sourceToggle = document.querySelector<HTMLButtonElement>(
    "[data-trellis-source-toggle]",
  );
  const form = document.querySelector<HTMLFormElement>(
    "[data-trellis-editor-form]",
  );
  const saveError = document.querySelector<HTMLElement>(
    "[data-trellis-save-error]",
  );
  const submitButton = form?.querySelector<HTMLButtonElement>(
    "button[type='submit']",
  );
  const initialDoc = trellisEditor ?? sourceView?.value ?? source?.value ?? "";
  // @ts-ignore
  const editor = new EditorView({
    // @ts-ignore
    doc: initialDoc,
    parent: el,
    extensions: [
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          const markdown = update.state.doc.toString();
          if (source) source.value = markdown;
          if (sourceView && sourceView.hidden) sourceView.value = markdown;
        }
      }),
      markdown({
        // @ts-ignore support for standard syntax highlighting in code fences
        codeLanguages: languages,
        extensions: [GFM, prosemarkMarkdownSyntaxExtensions],
      }),
      prosemarkBasicSetup(),
      prosemarkBaseThemeSetup(),
      prosemarkLightThemeSetup(),
      htmlBlockExtension, // render HTML blocks
    ],
  });

  if (source) source.value = initialDoc;
  if (sourceView) sourceView.value = initialDoc;

  sourceToggle?.addEventListener("click", () => {
    if (!sourceView) return;

    const showSource = sourceView.hidden;
    if (showSource) {
      const markdown = editor.state.doc.toString();
      sourceView.value = markdown;
      if (source) source.value = markdown;
    } else {
      const markdown = sourceView.value;
      editor.dispatch({
        changes: {
          from: 0,
          to: editor.state.doc.length,
          insert: markdown,
        },
      });
      if (source) source.value = markdown;
    }

    sourceView.hidden = !showSource;
    el.hidden = showSource;
    sourceToggle.setAttribute("aria-pressed", String(showSource));
  });

  sourceView?.addEventListener("input", () => {
    if (source) source.value = sourceView.value;
  });

  form?.addEventListener("submit", async (event) => {
    event.preventDefault();

    const saveUrl = form.dataset.saveUrl;
    if (!saveUrl) return;

    const markdown = sourceView && !sourceView.hidden
      ? sourceView.value
      : editor.state.doc.toString();
    if (source) source.value = markdown;

    if (saveError) {
      saveError.hidden = true;
      saveError.textContent = "";
    }
    if (submitButton) submitButton.disabled = true;

    try {
      const response = await fetch(saveUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify({ doc: markdown }),
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
