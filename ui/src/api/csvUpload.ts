interface UploadResult {
  path: string;
}

export async function uploadCsv(file: File): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const response = await fetch("/api/import/csv", {
    method: "POST",
    body: form,
  });
  if (!response.ok) {
    let error: { code: string; message: string } = {
      code: "import.upload_failed",
      message: `CSV upload failed with HTTP ${response.status}`,
    };
    try {
      const parsed = (await response.json()) as Partial<typeof error>;
      error = {
        code: parsed.code ?? error.code,
        message: parsed.message ?? error.message,
      };
    } catch {
      // Keep the transport fallback when a proxy returns a non-JSON body.
    }
    throw error;
  }
  const result = (await response.json()) as Partial<UploadResult>;
  if (typeof result.path !== "string" || result.path.length === 0) {
    throw { code: "import.invalid_upload_response", message: "The server did not return a CSV upload token." };
  }
  return result.path;
}
