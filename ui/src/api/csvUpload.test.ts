import { afterEach, describe, expect, it, vi } from "vitest";
import { uploadCsv } from "./csvUpload";

describe("uploadCsv", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("uploads the selected file as multipart and returns the opaque token", async () => {
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(JSON.stringify({ path: "8c48ac56-e0f7-4d8f-b835-ff3b0b39cfb9.csv" }), { status: 200 })
    );
    vi.stubGlobal("fetch", fetchMock);
    const file = new File(["date,amount\n2026-07-18,-4.50"], "history.csv", { type: "text/csv" });

    await expect(uploadCsv(file)).resolves.toBe("8c48ac56-e0f7-4d8f-b835-ff3b0b39cfb9.csv");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const call = fetchMock.mock.calls[0];
    if (!call) throw new Error("fetch was not called");
    const [url, init] = call;
    expect(url).toBe("/api/import/csv");
    expect(init?.method).toBe("POST");
    expect(init?.body).toBeInstanceOf(FormData);
    expect((init?.body as FormData).get("file")).toBe(file);
  });

  it("throws the server AppError when an upload is rejected", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        new Response(
          JSON.stringify({ code: "import.invalid_file_type", message: "the uploaded file must have a .csv extension" }),
          { status: 400 }
        )
      )
    );

    await expect(uploadCsv(new File(["x"], "notes.txt"))).rejects.toEqual({
      code: "import.invalid_file_type",
      message: "the uploaded file must have a .csv extension",
    });
  });
});
