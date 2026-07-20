import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ClarificationCard } from "./ClarificationCard";
import { useClarifications } from "../../../state/clarifications";

/**
 * A clarification is the one place the Copilot refuses to guess. These tests
 * pin the three properties that make it worth blocking on: the user can always
 * answer in their own words, they can always get out, and the composer is held
 * only until one of those happens.
 */

const append = vi.fn();
vi.mock("@assistant-ui/react", () => ({
  useThreadRuntime: () => ({ append }),
}));

type Option = { id: string; label: string; hint: string | null };

function block(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    kind: "clarification" as const,
    clarificationId: "c1",
    question: "Which account did you mean?",
    multiSelect: false,
    options: [] as Option[],
    textPlaceholder: null,
    referenceType: "account",
    ...overrides,
  } as Parameters<typeof ClarificationCard>[0]["block"];
}

const TWO_ACCOUNTS: Option[] = [
  { id: "a1", label: "Chequing", hint: "$1,240.00" },
  { id: "a2", label: "Savings", hint: "$8,000.00" },
];

beforeEach(() => {
  vi.clearAllMocks();
  useClarifications.setState({ resolved: {}, pending: null });
});

describe("ClarificationCard", () => {
  it("blocks the composer while unanswered", () => {
    render(<ClarificationCard block={block()} />);
    expect(useClarifications.getState().pending).toMatchObject({ id: "c1" });
  });

  it("offers the server-grounded options with their hints", () => {
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS })} />);
    expect(screen.getByRole("radio", { name: /chequing/i })).toBeInTheDocument();
    expect(screen.getByText("$1,240.00")).toBeInTheDocument();
  });

  it("sends the chosen option as an ordinary user turn", () => {
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS })} />);
    fireEvent.click(screen.getByRole("radio", { name: /savings/i }));
    fireEvent.click(screen.getByRole("button", { name: /use this/i }));

    // The hint rides along so the answer is unambiguous even when two
    // accounts share a name.
    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "Savings ($8,000.00)" }],
    });
    expect(useClarifications.getState().pending).toBeNull();
  });

  it("single-select replaces the choice rather than accumulating", () => {
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS })} />);
    fireEvent.click(screen.getByRole("radio", { name: /chequing/i }));
    fireEvent.click(screen.getByRole("radio", { name: /savings/i }));
    fireEvent.click(screen.getByRole("button", { name: /use this/i }));

    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "Savings ($8,000.00)" }],
    });
  });

  it("multi-select accumulates and sends every choice", () => {
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS, multiSelect: true })} />);
    fireEvent.click(screen.getByRole("checkbox", { name: /chequing/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /savings/i }));
    fireEvent.click(screen.getByRole("button", { name: /use these 2/i }));

    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "Chequing ($1,240.00), Savings ($8,000.00)" }],
    });
  });

  it("multi-select can deselect", () => {
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS, multiSelect: true })} />);
    const chequing = screen.getByRole("checkbox", { name: /chequing/i });
    fireEvent.click(chequing);
    fireEvent.click(chequing);
    expect(screen.getByRole("button", { name: /use this/i })).toBeDisabled();
  });

  it("keeps free text available even when options exist", () => {
    // The escape hatch: an option set that lacks the user's answer must never
    // be a trap.
    render(<ClarificationCard block={block({ options: TWO_ACCOUNTS })} />);
    const input = screen.getByLabelText("Answer");
    fireEvent.change(input, { target: { value: "my joint one" } });
    fireEvent.click(screen.getByRole("button", { name: /send answer/i }));

    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "my joint one" }],
    });
  });

  it("is free-text only when the server could ground no options", () => {
    render(<ClarificationCard block={block({ options: [] })} />);
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Answer")).toBeInTheDocument();
  });

  it("will not send a blank or whitespace answer", () => {
    render(<ClarificationCard block={block()} />);
    const input = screen.getByLabelText("Answer");
    fireEvent.change(input, { target: { value: "   " } });
    expect(screen.getByRole("button", { name: /send answer/i })).toBeDisabled();
    expect(append).not.toHaveBeenCalled();
  });

  it("can be dismissed without answering", () => {
    // A permanently blocked composer after an abandoned thread would be a trap.
    render(<ClarificationCard block={block()} />);
    fireEvent.click(screen.getByRole("button", { name: /dismiss question/i }));

    expect(useClarifications.getState().pending).toBeNull();
    expect(append).not.toHaveBeenCalled();
  });

  it("does not re-block once dealt with", () => {
    // The block stays in the thread forever; answering it must not mean
    // re-answering it on every render or reload.
    useClarifications.setState({ resolved: { c1: true }, pending: null });
    render(<ClarificationCard block={block()} />);

    expect(useClarifications.getState().pending).toBeNull();
    expect(screen.getByTestId("clarification-resolved")).toBeInTheDocument();
    expect(screen.queryByLabelText("Answer")).not.toBeInTheDocument();
  });

  it("still shows the question after it has been dealt with", () => {
    useClarifications.setState({ resolved: { c1: true }, pending: null });
    render(<ClarificationCard block={block()} />);
    expect(screen.getByText("Which account did you mean?")).toBeInTheDocument();
  });

  it("uses the model's placeholder when it supplied one", () => {
    render(<ClarificationCard block={block({ textPlaceholder: "e.g. the joint account" })} />);
    expect(screen.getByPlaceholderText("e.g. the joint account")).toBeInTheDocument();
  });

  it("stops blocking when it goes away unanswered", () => {
    // Switching conversation unmounts the card. If the block did not lift with
    // it, the next thread's composer would be stuck on a question that is no
    // longer on screen — and the dismiss button lives in this component, so
    // there would be no way out at all.
    const { unmount } = render(<ClarificationCard block={block()} />);
    expect(useClarifications.getState().pending).toMatchObject({ id: "c1" });

    unmount();

    expect(useClarifications.getState().pending).toBeNull();
    // Still unanswered, so revisiting that thread should block again.
    expect(useClarifications.getState().resolved.c1).toBeUndefined();
  });

  it("does not clear a different clarification's block when it unmounts", () => {
    const { unmount } = render(<ClarificationCard block={block()} />);
    // Another card took over the block in the meantime.
    useClarifications.setState({ pending: { id: "c2", question: "Which goal?" } });

    unmount();

    expect(useClarifications.getState().pending).toMatchObject({ id: "c2" });
  });

  it("sends the hint so identically-named options stay distinguishable", () => {
    // Account names have no uniqueness constraint. Sending the bare label
    // would hand the model the same string for either choice, putting it right
    // back to guessing.
    const twins: Option[] = [
      { id: "a1", label: "Everyday", hint: "$120.00" },
      { id: "a2", label: "Everyday", hint: "$4,500.00" },
    ];
    render(<ClarificationCard block={block({ options: twins })} />);
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    fireEvent.click(radios[1] as HTMLElement);
    fireEvent.click(screen.getByRole("button", { name: /use this/i }));

    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "Everyday ($4,500.00)" }],
    });
  });

  it("sends a bare label when there is no hint to add", () => {
    render(<ClarificationCard block={block({ options: [{ id: "a1", label: "Chequing", hint: null }] })} />);
    fireEvent.click(screen.getByRole("radio", { name: /chequing/i }));
    fireEvent.click(screen.getByRole("button", { name: /use this/i }));

    expect(append).toHaveBeenCalledWith({
      role: "user",
      content: [{ type: "text", text: "Chequing" }],
    });
  });

  it("keeps two identically-named accounts separately selectable", () => {
    // Account names have no uniqueness constraint; the id is what resolves.
    const twins: Option[] = [
      { id: "a1", label: "Everyday", hint: "$120.00" },
      { id: "a2", label: "Everyday", hint: "$4,500.00" },
    ];
    render(<ClarificationCard block={block({ options: twins })} />);
    expect(screen.getAllByRole("radio")).toHaveLength(2);
    expect(screen.getByText("$120.00")).toBeInTheDocument();
    expect(screen.getByText("$4,500.00")).toBeInTheDocument();
  });
});
