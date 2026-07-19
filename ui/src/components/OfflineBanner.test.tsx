import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import OfflineBanner from "./OfflineBanner";
import { useOnline } from "../pwa/useOnline";

type AnyRec = Record<string, unknown>;

vi.mock("../pwa/useOnline", () => ({
  useOnline: vi.fn(),
}));

describe("OfflineBanner", () => {
  afterEach(() => {
    vi.clearAllMocks();
    delete (window as unknown as AnyRec).__FINSIGHT_HTTP__;
  });

  it("server mode + offline: renders the offline banner", () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(useOnline).mockReturnValue(false);

    render(<OfflineBanner />);

    expect(screen.getByRole("status")).toHaveTextContent(
      /offline.*showing your last synced data.*changes are paused until you reconnect/i
    );
  });

  it("server mode + online: renders nothing", () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.mocked(useOnline).mockReturnValue(true);

    const { container } = render(<OfflineBanner />);

    expect(container).toBeEmptyDOMElement();
  });

  it("desktop mode: renders nothing even if useOnline reports offline", () => {
    vi.mocked(useOnline).mockReturnValue(false);

    const { container } = render(<OfflineBanner />);

    expect(container).toBeEmptyDOMElement();
  });
});
