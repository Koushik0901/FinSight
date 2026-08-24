import { useEffect, useMemo, useState } from "react";
import PageHeader from "../components/PageHeader";
import { isServerMode } from "../api/auth";
import SettingsAccounts from "./settings/SettingsAccounts";
import SettingsData, { PhilosophySection } from "./settings/SettingsData";
import SettingsAppearance from "./settings/SettingsAppearance";
import SettingsCurrency from "./settings/SettingsCurrency";
import SettingsShortcuts from "./settings/SettingsShortcuts";

export { PhilosophySection };

const SECTIONS = [
  ["profile", "Profile"],
  ["targets", "Financial targets"],
  ["philosophy", "How you want advice"],
  ["privacy", "Privacy & data"],
  ["backups", "Data & backups"],
  ["agent", "Agent"],
  ["provider", "AI Provider"],
  ["appearance", "Appearance"],
  ["currency", "Currency"],
  ["connections", "Connections"],
  ["notifications", "Notifications"],
  ["keyboard", "Keyboard"],
  ["about", "About"],
] as const;
const SERVER_ACCOUNT_SECTION = ["account", "Account"] as const;

function useActiveSection(ids: readonly string[]) {
  const [active, setActive] = useState<string>(ids[0] ?? "");
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;
    const visibleRatios = new Map<string, number>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          visibleRatios.set(entry.target.id, entry.isIntersecting ? entry.intersectionRatio : 0);
        }
        let bestId = "";
        let bestRatio = 0;
        for (const [id, ratio] of visibleRatios) {
          if (ratio > bestRatio) {
            bestRatio = ratio;
            bestId = id;
          }
        }
        if (bestId) setActive(bestId.replace(/^sec-/, ""));
      },
      { rootMargin: "-96px 0px -60% 0px", threshold: [0, 0.25, 0.5, 0.75, 1] }
    );
    const elements = ids
      .map((id) => document.getElementById(`sec-${id}`))
      .filter((el): el is HTMLElement => el !== null);
    elements.forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, [ids]);
  return active;
}

export default function Settings() {
  const serverMode = useMemo(() => isServerMode(), []);
  const sections = useMemo(() => (serverMode ? [...SECTIONS, SERVER_ACCOUNT_SECTION] : SECTIONS), [serverMode]);
  const sectionIds = useMemo(() => sections.map((s) => s[0] as string) as string[], [sections]);
  const activeSection = useActiveSection(sectionIds as unknown as readonly string[]);

  return (
    <div className="screen screen-settings">
      <PageHeader eyebrow="Settings" title="Make it yours." dot={false} />
      <div className="settings-layout">
        <nav className="settings-nav">
          {sections.map((entry) => {
            const [id, label] = entry as unknown as [string, string];
            return (
              <a key={id} href={`#sec-${id}`} className={`nav-item${activeSection === id ? " active" : ""}`}>
                {label}
              </a>
            );
          })}
        </nav>
        <div style={{ display: "flex", flexDirection: "column", gap: 56 }}>
          <SettingsAccounts />
          <SettingsData />
          <SettingsAppearance />
          <SettingsCurrency />
          <SettingsShortcuts />
        </div>
      </div>
    </div>
  );
}
