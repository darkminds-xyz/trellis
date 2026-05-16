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

function boot() {
  const el = document.getElementById("trellis-editor");
  if (!el) return;
  // @ts-ignore
  const editor = new EditorView({
    // @ts-ignore
    doc: trellisEditor,
    parent: el,
    extensions: [
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

  return editor;
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
