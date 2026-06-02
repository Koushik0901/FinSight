/* App shell — routes, tweaks, theme, command palette, onboarding overlay */

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "accent": "lime",
  "density": "cozy",
  "privacy": false
}/*EDITMODE-END*/;

/* Curated accent options (Maybe-style lime is default) */
const ACCENT_OPTIONS = [
  { id: "lime",   hex: "#C9F950", ink: "#0A0F02", dark: "#C9F950" },
  { id: "emerald",hex: "#34D399", ink: "#04130C", dark: "#34D399" },
  { id: "sky",    hex: "#60A5FA", ink: "#02101F", dark: "#60A5FA" },
  { id: "violet", hex: "#A78BFA", ink: "#0F0820", dark: "#A78BFA" },
  { id: "amber",  hex: "#FBBF24", ink: "#1A1300", dark: "#FBBF24" },
  { id: "rose",   hex: "#FB7185", ink: "#1F0710", dark: "#FB7185" },
];

function applyAccent(id) {
  const root = document.documentElement;
  const opt = ACCENT_OPTIONS.find(o => o.id === id) || ACCENT_OPTIONS[0];
  // hex to rgb for alpha mixes
  const hex = opt.hex.replace("#", "");
  const r = parseInt(hex.slice(0,2), 16);
  const g = parseInt(hex.slice(2,4), 16);
  const b = parseInt(hex.slice(4,6), 16);
  root.style.setProperty("--accent", opt.hex);
  root.style.setProperty("--accent-ink", opt.ink);
  root.style.setProperty("--accent-2", `rgba(${r}, ${g}, ${b}, 0.14)`);
  root.style.setProperty("--accent-3", `rgba(${r}, ${g}, ${b}, 0.28)`);
  root.style.setProperty("--accent-glow", `0 0 60px rgba(${r}, ${g}, ${b}, 0.20)`);
}

function App() {
  const [route, setRoute] = React.useState("today");
  const [cmdOpen, setCmdOpen] = React.useState(false);
  const [onbOpen, setOnbOpen] = React.useState(false);
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);

  // Expose for any component to navigate / open palette via window helpers
  React.useEffect(() => {
    window.openCmd = () => setCmdOpen(true);
    window.navigate = setRoute;
    return () => { delete window.openCmd; delete window.navigate; };
  }, []);

  // Apply theme + density + accent to <html>
  React.useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", t.theme);
    root.setAttribute("data-density", t.density);
    root.setAttribute("data-privacy", t.privacy ? "on" : "off");
    applyAccent(t.accent);
  }, [t.theme, t.density, t.privacy, t.accent]);

  // Keyboard shortcuts
  React.useEffect(() => {
    const onKey = (e) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key.toLowerCase() === "k") { e.preventDefault(); setCmdOpen(o => !o); }
      else if (meta && e.key === ".") { e.preventDefault(); setTweak({ privacy: !t.privacy }); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [t.privacy]);

  const screens = {
    today:        <Today setRoute={setRoute} />,
    insights:     <Insights />,
    accounts:     <Accounts setRoute={setRoute} />,
    transactions: <Transactions />,
    budget:       <Budget />,
    categories:   <Categories />,
    recurring:    <Recurring />,
    goals:        <Goals />,
    scenarios:    <Scenarios />,
    reports:      <Reports />,
    rules:        <Rules />,
    settings:     <Settings />,
  };

  const currentAccentHex = (ACCENT_OPTIONS.find(o => o.id === t.accent) || ACCENT_OPTIONS[0]).hex;

  return (
    <div className="app">
      <Sidebar route={route} setRoute={setRoute} openCmd={() => setCmdOpen(true)} openOnboarding={() => setOnbOpen(true)} />

      <main className="main" key={route}>
        <div className="main-inner">
          {screens[route]}
        </div>
      </main>

      <CommandPalette open={cmdOpen} onClose={() => setCmdOpen(false)} setRoute={setRoute} />

      {onbOpen && <Onboarding onDone={() => { setOnbOpen(false); setRoute("today"); }} />}

      {t.privacy && (
        <div className="privacy-badge">
          <I.EyeOff width="14" height="14" />
          <span>Privacy mode · ⌘.</span>
        </div>
      )}

      <Toaster />

      <TweaksPanel>
        <TweakSection label="Appearance" />
        <TweakRadio  label="Theme"   value={t.theme}   options={["light", "dark"]}     onChange={v => setTweak({ theme: v })} />
        <TweakRadio  label="Density" value={t.density} options={["cozy", "compact"]}   onChange={v => setTweak({ density: v })} />
        <TweakColor
          label="Accent"
          value={currentAccentHex}
          options={ACCENT_OPTIONS.map(o => o.hex)}
          onChange={(hex) => {
            const opt = ACCENT_OPTIONS.find(o => o.hex.toLowerCase() === String(hex).toLowerCase());
            if (opt) setTweak({ accent: opt.id });
          }}
        />
        <TweakSection label="Privacy" />
        <TweakToggle label="Hide amounts (⌘.)" value={t.privacy} onChange={v => setTweak({ privacy: v })} />
        <TweakSection label="Onboarding" />
        <TweakButton label="Show first-run setup" onClick={() => setOnbOpen(true)} />
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
