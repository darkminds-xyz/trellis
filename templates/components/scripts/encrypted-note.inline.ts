const GLYPHS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&*+<>=?@^_~";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

type DecodePlanStep = {
  stable: boolean;
  target: string;
  settleAt?: number;
  noise?: number;
};

type EncryptedDataset = DOMStringMap & {
  salt: string;
  nonce: string;
  ciphertext: string;
  iterations?: string;
};

function glyphLine(length = 48): string {
  const len = Math.max(24, Math.min(260, length));
  let out = "";
  for (let i = 0; i < len; i++) {
    out += GLYPHS[(Math.random() * GLYPHS.length) | 0];
  }
  return out;
}

function glyphFromCipher(ciphertext = ""): string {
  if (!ciphertext) return glyphLine();

  const chars = [];
  for (let i = 0; i < ciphertext.length; i++) {
    chars.push(GLYPHS[(Math.random() * GLYPHS.length) | 0]);
    // insert a soft break every ~64 chars to mimic paragraph wrapping
    if ((i + 1) % 64 === 0) chars.push("\n");
  }
  return chars.join("");
}

function b64ToBytes(b64: string): Uint8Array {
  if (!b64) return new Uint8Array();
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function extractText(html: string): string {
  const tmp = document.createElement("div");
  tmp.innerHTML = html;
  return tmp.textContent || "";
}

function makePlan(text: string): DecodePlanStep[] {
  const nonSpace = text.replace(/\s/g, "").length || 1;
  const plan: DecodePlanStep[] = [];
  let logical = 0;

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (/\s/.test(ch)) {
      plan.push({ stable: true, target: ch });
      continue;
    }

    const wave = logical / nonSpace;
    const settleAt = Math.min(1, Math.max(0.05, wave + (Math.random() - 0.5) * 0.2));
        const noise = 0.35 + (1 - wave) * 0.55; // louder early, calmer later
        plan.push({ stable: false, target: ch, settleAt, noise });
    logical++;
  }
  return plan;
}

function runDecodeEffect(
  outEl: HTMLElement | null,
  targetText: string
): Promise<void> {
  if (!outEl) return Promise.resolve();

  const plan = makePlan(targetText);
  const duration = 1200;
  let current = targetText
    .split("")
    .map((ch) => (ch.match(/\s/) ? ch : GLYPHS[(Math.random() * GLYPHS.length) | 0]));

  outEl.hidden = false;
  outEl.textContent = current.join("");

  return new Promise<void>((resolve) => {
    const start = performance.now();

    const tick = (ts: number) => {
      const t = Math.min(1, (ts - start) / duration);

      for (let i = 0; i < plan.length; i++) {
        const step = plan[i];
        if (step.stable) {
          current[i] = step.target;
          continue;
        }

        const settleAt = step.settleAt ?? 1;
        const noise = step.noise ?? 0.35;

        if (t >= settleAt) {
          current[i] = step.target;
          continue;
        }

        const remaining = (settleAt - t) / Math.max(settleAt, 1e-6);
        const pFlip = Math.min(0.98, noise * (0.25 + remaining));
        const pTrue = 0.06 + (1 - remaining) * 0.12;
        const r = Math.random();
        if (r < pTrue) current[i] = step.target;
        else if (r < pTrue + pFlip) current[i] = GLYPHS[(Math.random() * GLYPHS.length) | 0];
      }

      outEl.textContent = current.join("");

      if (t < 1) requestAnimationFrame(tick);
      else {
        outEl.textContent = targetText;
        resolve();
      }
    };

    requestAnimationFrame(tick);
  });
}

async function decryptPayload(
  dataset: EncryptedDataset,
  password: string
): Promise<string> {
  const salt = b64ToBytes(dataset.salt);
  const nonce = b64ToBytes(dataset.nonce);
  const ciphertext = b64ToBytes(dataset.ciphertext);
  const saltBuf = salt.buffer as ArrayBuffer;
  const nonceBuf = nonce.buffer as ArrayBuffer;
  const cipherBuf = ciphertext.buffer as ArrayBuffer;

  if (!salt.length || !nonce.length || !ciphertext.length) {
    throw new Error("Missing cipher payload");
  }

  const iterations = Number(dataset.iterations) || 120000;

  const baseKey = await crypto.subtle.importKey(
    "raw",
    encoder.encode(password),
    "PBKDF2",
    false,
    ["deriveKey"]
  );

  const aesKey = await crypto.subtle.deriveKey(
    {
      name: "PBKDF2",
      salt: saltBuf,
      iterations,
      hash: "SHA-256",
    },
    baseKey,
    { name: "AES-GCM", length: 256 },
    false,
    ["decrypt"]
  );

  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: nonceBuf },
    aesKey,
    cipherBuf
  );

  return decoder.decode(plaintext);
}

function initEncryptedNote(note: HTMLElement) {
  const status = note.querySelector<HTMLElement>(".encrypted-note__status");
  const decode = note.querySelector<HTMLElement>(".encrypted-note__decode");
  const body = note.querySelector<HTMLElement>(".encrypted-note__body");
  const form = note.querySelector<HTMLFormElement>(".encrypted-note__form");
  const scrambleBtn = note.querySelector<HTMLButtonElement>(
    ".encrypted-note__scramble"
  );

  const setStatus = (msg: string) => {
    if (status) status.textContent = msg;
  };

  if (!window.crypto?.subtle) {
    setStatus("Cannot decrypt: WebCrypto not available in this browser.");
    return;
  }

  if (decode) {
    const placeholder = glyphFromCipher(note.dataset.ciphertext || "");
    decode.dataset.placeholder = placeholder;
    decode.textContent = placeholder;
  }

  scrambleBtn?.addEventListener("click", () => {
    if (!decode) return;
    const seedLength = (decode.textContent || decode.dataset.placeholder || "").length || 48;
    decode.textContent = glyphLine(seedLength);
  });

  let unlocked = false;

  form?.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (unlocked) return;

    const data = new FormData(form);
    const password = (data.get("password") || "").toString();
    if (!password) {
      setStatus("Enter a password to decrypt");
      note.classList.add("is-error");
      return;
    }

    note.classList.remove("is-error");
    setStatus("Decrypting…");

    try {
      const plaintext = await decryptPayload(
        note.dataset as EncryptedDataset,
        password
      );
      const textOnly = extractText(plaintext) || "Decrypted";

      await runDecodeEffect(decode, textOnly);

      if (body) {
        body.innerHTML = plaintext;
        body.hidden = false;
      }

      if (decode) {
        decode.textContent = "";
        decode.hidden = true;
      }

      if (form) {
        form.remove();
      }

      const chrome = note.querySelector(".encrypted-note__chrome");
      if (chrome && chrome.children.length === 0) {
        chrome.remove();
      }

      if (status) {
        status.remove();
      }

      note.classList.add("is-unlocked");
      setStatus("Decrypted");
      unlocked = true;
    } catch (err) {
      console.warn("Failed to decrypt note", err);
      setStatus("Incorrect password or corrupted payload");
      note.classList.add("is-error");
    }
  });
}

document.addEventListener("DOMContentLoaded", () => {
  document
    .querySelectorAll<HTMLElement>(".encrypted-note")
    .forEach((note) => initEncryptedNote(note));
});
