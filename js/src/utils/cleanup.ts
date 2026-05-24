type CleanupWindow = Window & {
  addCleanup?: (fn: () => void) => void;
};

export const registerCleanup: ((fn: () => void) => void) | null =
  typeof window !== "undefined" &&
  typeof (window as CleanupWindow).addCleanup === "function"
    ? ((window as CleanupWindow).addCleanup ?? null)
    : null;

export function addCleanup(cleanup: () => void) {
  registerCleanup?.(cleanup);
}
