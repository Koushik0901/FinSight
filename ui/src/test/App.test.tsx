import { fireEvent, render, screen, within } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "../App";

describe("App", () => {
  it("keeps the core destinations visible and specialists under More", () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    );
    // Scoped to the desktop sidebar — jsdom doesn't apply the ≤900px media
    // query that hides it in favor of BottomNav, so both render at once and
    // share several tab labels.
    const sidebar = within(screen.getByLabelText("Primary navigation"));
    expect(sidebar.getByText("Today")).toBeInTheDocument();
    expect(sidebar.getByText("Accounts")).toBeInTheDocument();
    expect(sidebar.getByText("Budget")).toBeInTheDocument();
    expect(sidebar.getByText("Goals")).toBeInTheDocument();
    expect(sidebar.getByText("Reports")).toBeInTheDocument();
    expect(sidebar.getByText("Copilot")).toBeInTheDocument();
    expect(sidebar.getByText("Settings")).toBeInTheDocument();

    expect(sidebar.queryByText("Categories")).not.toBeInTheDocument();
    fireEvent.click(sidebar.getByRole("button", { name: "More" }));
    expect(sidebar.getByText("Review")).toBeInTheDocument();
    expect(sidebar.getByText("Categories")).toBeInTheDocument();
    expect(sidebar.getByText("Rules & automation")).toBeInTheDocument();
  });
});
