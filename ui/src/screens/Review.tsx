import { useSearchParams } from "react-router-dom";
import PageHeader from "../components/PageHeader";
import Inbox from "./Inbox";
import CategoryReview from "./CategoryReview";

export default function Review() {
  const [params, setParams] = useSearchParams();
  const view = params.get("view") === "categories" ? "categories" : "attention";

  const chooseView = (next: "attention" | "categories") => {
    const updated = new URLSearchParams(params);
    if (next === "categories") updated.set("view", "categories");
    else updated.delete("view");
    setParams(updated, { replace: true });
  };

  return (
    <div className="screen screen-review">
      <PageHeader
        eyebrow="Review"
        title="Decisions waiting for you."
        description="Handle alerts, import questions, and uncertain categories in one place."
      />
      <div className="toolbar review-hub-tabs" role="tablist" aria-label="Review type">
        <button type="button" role="tab" aria-selected={view === "attention"} className={view === "attention" ? "on" : ""} onClick={() => chooseView("attention")}>Attention</button>
        <button type="button" role="tab" aria-selected={view === "categories"} className={view === "categories" ? "on" : ""} onClick={() => chooseView("categories")}>Category decisions</button>
      </div>
      {view === "attention" ? <Inbox embedded /> : <CategoryReview embedded />}
    </div>
  );
}
