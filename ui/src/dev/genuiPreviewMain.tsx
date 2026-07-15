/**
 * DEV-only isolated mount for the generative-UI block gallery.
 *
 * Renders <GenUiPreview/> on its own — no BrowserRouter, no App shell, no
 * onboarding redirect, no Toaster, no animated hero. That keeps the page
 * static so it renders (and screenshots) cleanly, while still exercising the
 * REAL FinSightResponseBlock dispatcher and cards. Served in dev at
 * /genui-preview.html; never part of the production bundle (only index.html is
 * a build entry).
 */
import { createRoot } from "react-dom/client";
import GenUiPreview from "./GenUiPreview";
import "../styles/reset.css";
import "../styles/tokens.css";
import "../styles/app.css";

createRoot(document.getElementById("genui-root")!).render(<GenUiPreview />);
