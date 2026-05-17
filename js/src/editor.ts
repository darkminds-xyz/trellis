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

  return editor;
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
