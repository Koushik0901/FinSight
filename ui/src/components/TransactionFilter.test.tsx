import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import TransactionFilter from "./TransactionFilter";
import type { TxnFilterInput } from "../api/client";

const baseFilter: TxnFilterInput = {
  accountId: null,
  limit: null,
  offset: null,
  search: null,
  filterPreset: null,
  startDate: null,
  endDate: null,
};

describe("TransactionFilter", () => {
  it("calls onChange when search input changes", () => {
    const onChange = vi.fn();
    render(<TransactionFilter value={baseFilter} onChange={onChange} />);
    const input = screen.getByLabelText("Search transactions");
    fireEvent.change(input, { target: { value: "coffee" } });
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, search: "coffee" });
  });

  it("calls onChange when start date changes", () => {
    const onChange = vi.fn();
    render(<TransactionFilter value={baseFilter} onChange={onChange} />);
    const input = screen.getByLabelText("Start date");
    fireEvent.change(input, { target: { value: "2026-01-01" } });
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, startDate: "2026-01-01" });
  });

  it("calls onChange when end date changes", () => {
    const onChange = vi.fn();
    render(<TransactionFilter value={baseFilter} onChange={onChange} />);
    const input = screen.getByLabelText("End date");
    fireEvent.change(input, { target: { value: "2026-01-31" } });
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, endDate: "2026-01-31" });
  });

  it("calls onChange with preset values", () => {
    const onChange = vi.fn();
    render(
      <TransactionFilter
        value={baseFilter}
        onChange={onChange}
        counts={{ review: 5, anomalies: 2 }}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /Needs review 5/ }));
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: "needs_review" });

    fireEvent.click(screen.getByRole("button", { name: /Anomalies 2/ }));
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: "anomalies" });

    fireEvent.click(screen.getByRole("button", { name: /^All$/ }));
    expect(onChange).toHaveBeenCalledWith({ ...baseFilter, filterPreset: null });
  });
});
