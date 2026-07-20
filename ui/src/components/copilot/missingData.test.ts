import { describe, it, expect } from "vitest";
import { buildMetaFromMessages } from "./TauriRuntime";
import type { ConversationMessage } from "../../api/client";

/**
 * Missing-data items ride two paths to the screen: the live `Done` frame, and
 * the metadata blob replayed when a conversation is reloaded. The blob is JSON
 * read back out of the database, so it is the one that has to be parsed
 * defensively — a message written by an older build has no `missingData` at
 * all, and a partially-written one could carry anything.
 *
 * The rule these tests pin: an item keeps its call-to-action only when both
 * halves survive. A labelled button with nowhere to go is worse than plain
 * text, because it looks like it works.
 */

function message(metadataJson: string | null): ConversationMessage {
  return {
    id: "msg-1",
    conversationId: "conv-1",
    role: "assistant",
    content: "Here is what I found.",
    toolTrace: null,
    actionBundleId: null,
    branchParentId: null,
    partsJson: null,
    runStatus: "complete",
    agUiMetadataJson: metadataJson,
    createdAt: "2026-07-19T00:00:00Z",
  } as unknown as ConversationMessage;
}

function metaFor(metadata: unknown) {
  return buildMetaFromMessages([message(JSON.stringify(metadata))])["msg-1"];
}

describe("missing-data rehydration", () => {
  it("restores an actionable item across a reload", () => {
    const meta = metaFor({
      missingData: [
        {
          message: "Visa is missing APR.",
          actionLabel: "Add APR",
          actionPath: "/accounts?focusAccount=a1",
        },
      ],
    });
    expect(meta?.missingData).toHaveLength(1);
    expect(meta?.missingData?.[0]).toEqual({
      message: "Visa is missing APR.",
      actionLabel: "Add APR",
      actionPath: "/accounts?focusAccount=a1",
    });
  });

  it("restores a prose-only item with no call to action", () => {
    // The deep reasoning path lets the model author these, so there is no
    // entity to link to. The message must still survive.
    const meta = metaFor({
      missingData: [{ message: "Add APRs before finalizing.", actionLabel: null, actionPath: null }],
    });
    expect(meta?.missingData?.[0]).toEqual({
      message: "Add APRs before finalizing.",
      actionLabel: null,
      actionPath: null,
    });
  });

  it("drops half a call to action rather than rendering a dead button", () => {
    const meta = metaFor({
      missingData: [
        { message: "Label but no path", actionLabel: "Go", actionPath: null },
        { message: "Path but no label", actionLabel: null, actionPath: "/accounts" },
        { message: "Blank halves", actionLabel: "  ", actionPath: "  " },
      ],
    });
    expect(meta?.missingData).toHaveLength(3);
    for (const item of meta?.missingData ?? []) {
      expect(item.actionLabel).toBeNull();
      expect(item.actionPath).toBeNull();
    }
  });

  it("discards entries that are not usable at all", () => {
    const meta = metaFor({
      missingData: [
        { message: "Real one" },
        { message: "" },
        { message: "   " },
        { notAMessage: true },
        null,
        "a bare string",
        42,
      ],
    });
    expect(meta?.missingData).toHaveLength(1);
    expect(meta?.missingData?.[0]?.message).toBe("Real one");
  });

  it("handles a message written before the field existed", () => {
    const meta = metaFor({ toolTrace: ["get_liabilities"], followUpQuestions: [] });
    expect(meta?.missingData).toBeUndefined();
  });

  it("ignores a non-array missingData without throwing", () => {
    expect(() => metaFor({ missingData: "not an array" })).not.toThrow();
    expect(metaFor({ missingData: "not an array" })?.missingData).toBeUndefined();
  });

  it("survives metadata that is not valid JSON", () => {
    expect(() => buildMetaFromMessages([message("{{{ not json")])).not.toThrow();
  });

  it("survives a message with no metadata at all", () => {
    expect(() => buildMetaFromMessages([message(null)])).not.toThrow();
  });
});
