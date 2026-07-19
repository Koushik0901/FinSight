import { render, screen, within } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "../App";

describe("App", () => {
  it("renders sidebar with all routes", () => {
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
    // share several tab labels (Today, Inbox, Accounts, Settings).
    const sidebar = within(screen.getByLabelText("Primary navigation"));
    expect(sidebar.getByText("Today")).toBeInTheDocument();
    expect(sidebar.getByText("Inbox")).toBeInTheDocument();
    expect(sidebar.getByText("Accounts")).toBeInTheDocument();
    expect(sidebar.getByText("Categories")).toBeInTheDocument();
    expect(sidebar.getByText("Rules & agents")).toBeInTheDocument();
    expect(sidebar.getByText("Settings")).toBeInTheDocument();
  });
});
