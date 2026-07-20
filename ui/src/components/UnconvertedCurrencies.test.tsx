import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { UnconvertedCurrencies } from "./UnconvertedCurrencies";

describe("UnconvertedCurrencies", () => {
  it("stays silent for the single-currency user", () => {
    // Almost everyone. A caveat here would be noise about a problem they
    // don't have.
    const { container } = render(
      <UnconvertedCurrencies holdings={[]} primary="USD" />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing before metrics have loaded", () => {
    const { container } = render(
      <UnconvertedCurrencies holdings={undefined} primary={undefined} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("names the excluded money in its own currency, not the headline one", () => {
    render(
      <UnconvertedCurrencies
        holdings={[{ code: "CAD", accountCount: 2, balanceCents: 418_000 }]}
        primary="USD"
      />,
    );
    const note = screen.getByRole("status");
    expect(note).toHaveTextContent("Totals are in USD");
    // CA$ — rendering this as "$4,180" would imply it had been converted.
    expect(note).toHaveTextContent("CA$4,180");
    expect(note).toHaveTextContent(/not converted/i);
  });

  it("lists every additional currency, not just the largest", () => {
    render(
      <UnconvertedCurrencies
        holdings={[
          { code: "CAD", accountCount: 1, balanceCents: 418_000 },
          { code: "GBP", accountCount: 1, balanceCents: 90_000 },
        ]}
        primary="USD"
      />,
    );
    const note = screen.getByRole("status");
    expect(note).toHaveTextContent("CA$4,180");
    expect(note).toHaveTextContent("£900");
  });

  it("degrades to a generic label when no primary currency is known", () => {
    render(
      <UnconvertedCurrencies
        holdings={[{ code: "CAD", accountCount: 1, balanceCents: 1000 }]}
        primary={null}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("your main currency");
  });
});
