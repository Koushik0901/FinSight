import type { ReactNode } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Drawer from "../components/Drawer";

vi.mock("react-focus-lock", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

describe("Drawer", () => {
  it("renders title and children when open", () => {
    render(
      <Drawer open onClose={() => {}} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    expect(screen.getByRole("dialog", { name: /add account/i })).toBeInTheDocument();
    expect(screen.getByText("BODY")).toBeInTheDocument();
  });

  it("does not render content when closed", () => {
    render(
      <Drawer open={false} onClose={() => {}} title="Closed">
        <div>HIDDEN</div>
      </Drawer>
    );
    expect(screen.queryByText("HIDDEN")).not.toBeInTheDocument();
  });

  it("calls onClose when Escape is pressed", async () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("calls onClose when backdrop is clicked", () => {
    const onClose = vi.fn();
    render(
      <Drawer open onClose={onClose} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    fireEvent.click(screen.getByTestId("drawer-backdrop"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("sets aria-modal and labelledby", () => {
    render(
      <Drawer open onClose={() => {}} title="Add account">
        <div>BODY</div>
      </Drawer>
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog.getAttribute("aria-labelledby")).toBeTruthy();
  });
});
