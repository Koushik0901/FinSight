/** Triggers a browser/webview file download from in-memory string content —
 *  the replacement for the old native-save-dialog Tauri commands (Phase 4).
 *  Works identically in a plain browser tab, the installed PWA, and the thin
 *  desktop shell (all three now load the SAME ui/dist bundle; none of them
 *  has a native file-save dialog to call). */
export function downloadBlob(content: string, mimeType: string, filename: string): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
