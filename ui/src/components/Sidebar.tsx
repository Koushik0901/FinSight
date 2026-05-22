import { NavLink } from "react-router-dom";
import { ROUTES } from "../state/routes";

export function Sidebar() {
  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <h1>FinSight</h1>
      <nav>
        {ROUTES.map((r) => (
          <NavLink key={r.id} to={r.path} end={r.path === "/"}>
            {r.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
