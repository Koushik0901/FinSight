import { useTweaks, ACCENTS, type AccentId } from "../../state/tweaks";
import { Section } from "./Section";

export default function SettingsAppearance() {
  const { theme, density, accent, setTheme, setDensity, setAccent } = useTweaks();
  return (
    <Section id="appearance" title="Appearance" description="Theme, density, accent, and currency.">
      <div className="s-row">
        <div>
          <div className="label">Theme</div>
          <div className="desc">Switch between dark and light modes.</div>
        </div>
        <div className="toolbar">
          <button className={theme === "dark" ? "on" : ""} type="button" onClick={() => setTheme("dark")}>
            Dark
          </button>
          <button className={theme === "light" ? "on" : ""} type="button" onClick={() => setTheme("light")}>
            Light
          </button>
        </div>
        <div />
      </div>
      <div className="s-row">
        <div>
          <div className="label">Density</div>
          <div className="desc">Use cozy spacing or fit more on screen.</div>
        </div>
        <div className="toolbar">
          <button className={density === "cozy" ? "on" : ""} type="button" onClick={() => setDensity("cozy")}>
            Cozy
          </button>
          <button className={density === "compact" ? "on" : ""} type="button" onClick={() => setDensity("compact")}>
            Compact
          </button>
        </div>
        <div />
      </div>
      <div className="s-row">
        <div>
          <div className="label">Accent</div>
          <div className="desc">Pick the accent used in hero states and active controls.</div>
        </div>
        <div className="row row-sm wrap">
          {(Object.entries(ACCENTS) as [AccentId, { hex: string }][]).map(([id, value]) => (
            <button
              key={id}
              type="button"
              aria-label={id}
              onClick={() => setAccent(id)}
              style={{
                width: 28,
                height: 28,
                borderRadius: 999,
                background: value.hex,
                border: accent === id ? "2px solid var(--ink)" : "1px solid var(--line)",
              }}
            />
          ))}
        </div>
        <div />
      </div>
    </Section>
  );
}
