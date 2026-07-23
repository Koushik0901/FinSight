/**
 * Server-mode auth scene — the shared shell, floating-label field, and animated
 * showcase, ported from the "Plutus" Claude Design login into the real app.
 * SetupScreen / LoginScreen / RecoverScreen render their form bodies inside
 * {@link AuthShell}. All styling is scoped under `.fs-auth` (see auth.css) so the
 * design's generic class names don't collide with the app's globals.
 *
 * The showcase (right pane) is decorative: its figures and the "Mira & Adam"
 * household are an illustrative demo, not the signed-in user's data.
 */
import {
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type SVGProps,
} from "react";
import { toast } from "sonner";
import "../../styles/auth.css";

/* ── icons ─────────────────────────────────────────────── */
type IcoProps = SVGProps<SVGSVGElement>;
export const Ico = {
  user: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><circle cx="8" cy="5.5" r="2.6" /><path d="M3 13.2a5 5 0 0 1 10 0" /></svg>
  ),
  lock: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><rect x="3" y="7" width="10" height="7" rx="1.6" /><path d="M5 7V5a3 3 0 0 1 6 0v2" /></svg>
  ),
  key: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><circle cx="5.5" cy="5.5" r="3" /><path d="m7.6 7.6 5 5M11 11l1.4-1.4M9.6 9.6 11 8.2" /></svg>
  ),
  eye: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" /><circle cx="8" cy="8" r="1.8" /></svg>
  ),
  eyeoff: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M3 3l10 10" /><path d="M6 6.2C3.8 7.4 1.5 8 1.5 8s2.5 4.5 6.5 4.5c1.1 0 2.1-.3 3-.7" /><path d="M9.6 4.1A6.7 6.7 0 0 1 14.5 8s-.7 1.3-2 2.5" /></svg>
  ),
  check: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="m3 8 3.5 3.5L13 5" /></svg>
  ),
  arrow: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M3 8h9M8.5 4l4 4-4 4" /></svg>
  ),
  warn: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M8 5.5v3.5M8 11.2v.1" /><circle cx="8" cy="8" r="6.2" /></svg>
  ),
  spark: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M8 2v3M8 11v3M2 8h3M11 8h3M4.2 4.2l2 2M9.8 9.8l2 2M4.2 11.8l2-2M9.8 6.2l2-2" /></svg>
  ),
};

/* ── floating-label field ──────────────────────────────── */
export function Field({
  icon,
  label,
  type = "text",
  value,
  onChange,
  autoComplete,
  error,
  valid,
  required,
  autoFocus,
  id,
  trailing,
}: {
  icon: ReactNode;
  label: string;
  type?: string;
  value: string;
  onChange: (v: string) => void;
  autoComplete?: string;
  error?: string | null;
  valid?: boolean;
  required?: boolean;
  autoFocus?: boolean;
  id?: string;
  trailing?: ReactNode;
}) {
  const [focus, setFocus] = useState(false);
  const filled = value.length > 0;
  const cls = ["field", focus && "focused", filled && "filled", error && "invalid", valid && !error && "valid"]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={cls}>
      <div className="in-wrap">
        <div className="lead-ico">{icon}</div>
        <input
          id={id}
          type={type}
          value={value}
          autoComplete={autoComplete}
          autoFocus={autoFocus}
          placeholder={label}
          aria-label={label}
          aria-invalid={!!error}
          onChange={(e) => onChange(e.target.value)}
          onFocus={() => setFocus(true)}
          onBlur={() => setFocus(false)}
        />
        <label className="lab" htmlFor={id}>{label}{required && <b> *</b>}</label>
        <div className="trail">
          {valid && !error && <span className="valid-check">{Ico.check()}</span>}
          {trailing}
        </div>
      </div>
      {error && <div className="field-err">{Ico.warn()} {error}</div>}
    </div>
  );
}

/* ── password strength ─────────────────────────────────── */
export function strength(pw: string): number {
  let s = 0;
  if (pw.length >= 8) s++;
  if (/[A-Z]/.test(pw) && /[a-z]/.test(pw)) s++;
  if (/\d/.test(pw)) s++;
  if (/[^A-Za-z0-9]/.test(pw)) s++;
  return Math.min(s, 4);
}
export const STR = [
  { lab: "TOO SHORT", c: "var(--ink-faint)", hint: "8+ characters" },
  { lab: "WEAK", c: "#FB7185", hint: "Add a number" },
  { lab: "FAIR", c: "#FBBF24", hint: "Add a symbol" },
  { lab: "GOOD", c: "#60A5FA", hint: "Nearly there" },
  { lab: "STRONG", c: "#4ADE80", hint: "Great password" },
];

export function PasswordStrength({ pw, open }: { pw: string; open: boolean }) {
  const st = strength(pw);
  const meta = STR[st]!;
  return (
    <div className={"collapse" + (open ? " open" : "")}>
      <div className="collapse-inner">
        <div className="pw-strength">
          <div className="pw-bars">
            {[0, 1, 2, 3].map((i) => (
              <span key={i} style={{ background: i < st ? meta.c : undefined }} />
            ))}
          </div>
          <div className="pw-meta">
            <span className="lvl" style={{ color: meta.c }}>{meta.lab}</span>
            <span className="hint">{meta.hint}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ── count-up hook ─────────────────────────────────────── */
function useCountUp(target: number, dur = 1600, delay = 200): number {
  const [v, setV] = useState(0);
  useEffect(() => {
    let raf = 0;
    let t0 = 0;
    const to = window.setTimeout(() => {
      const tick = (t: number) => {
        if (!t0) t0 = t;
        const p = Math.min((t - t0) / dur, 1);
        const e = 1 - Math.pow(1 - p, 3);
        setV(target * e);
        if (p < 1) raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    }, delay);
    return () => { clearTimeout(to); cancelAnimationFrame(raf); };
  }, [target, dur, delay]);
  return v;
}
const money = (n: number) => Math.round(n).toLocaleString("en-US");

/* Shared reveal timing — each figure's line draw and its count-up use the SAME
   delay + duration + easing, so the line and the number start and finish
   together. Staggered so the three cards animate in sequence. */
const REVEAL = {
  hero: { delay: 500, dur: 2200 },
  checking: { delay: 850, dur: 2000 },
  house: { delay: 1050, dur: 2000 },
};
// cubic ease-out — the exact curve useCountUp applies to its value.
const easeOut = (p: number) => 1 - Math.pow(1 - p, 3);

// Distinct simulated stories (see the cards): a checking account that sits flat
// then takes one massive deposit; a savings account that climbs steadily.
const JOINT_CHECKING = [9200, 9450, 9150, 9600, 9350, 9700, 9500, 9750, 13600, 14820];
const HOUSE_FUND = [26800, 27050, 27250, 27500, 27780, 28040, 28320, 28640];

/* Catmull-Rom resample → a smooth curve through the points (no stair-steps). */
function smoothCurve(pts: number[], samples = 200): number[] {
  const n = pts.length;
  if (n < 3) return pts.slice();
  const out: number[] = [];
  for (let i = 0; i < samples; i++) {
    const u = (i / (samples - 1)) * (n - 1);
    const seg = Math.min(Math.floor(u), n - 2);
    const t = u - seg;
    const p0 = pts[Math.max(0, seg - 1)]!, p1 = pts[seg]!, p2 = pts[seg + 1]!, p3 = pts[Math.min(n - 1, seg + 2)]!;
    const t2 = t * t, t3 = t2 * t;
    out.push(0.5 * (2 * p1 + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 + (-p0 + 3 * p1 - 3 * p2 + p3) * t3));
  }
  return out;
}

/* ── mini sparkline ────────────────────────────────────── */
function Sparkline({ points, color, delay, dur }: { points: number[]; color: string; delay: number; dur: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    const pts = smoothCurve(points, 180);
    const min = Math.min(...pts), max = Math.max(...pts);
    const pad = 3;
    const X = (i: number) => (i / (pts.length - 1)) * w;
    const Y = (val: number) => h - pad - ((val - min) / (max - min || 1)) * (h - pad * 2);
    let raf = 0, to = 0, start = 0;
    const draw = (now: number) => {
      if (!start) start = now;
      const p = Math.min((now - start) / dur, 1);
      const n = Math.max(2, Math.floor(easeOut(p) * pts.length));
      ctx.clearRect(0, 0, w, h);
      ctx.save();
      const g = ctx.createLinearGradient(0, 0, 0, h);
      g.addColorStop(0, color + "44"); g.addColorStop(1, color + "00");
      ctx.beginPath(); ctx.moveTo(X(0), Y(pts[0]!));
      for (let i = 1; i < n; i++) ctx.lineTo(X(i), Y(pts[i]!));
      ctx.lineTo(X(n - 1), h); ctx.lineTo(0, h); ctx.closePath();
      ctx.fillStyle = g; ctx.fill();
      ctx.restore();
      ctx.beginPath(); ctx.moveTo(X(0), Y(pts[0]!));
      for (let i = 1; i < n; i++) ctx.lineTo(X(i), Y(pts[i]!));
      ctx.strokeStyle = color; ctx.lineWidth = 1.8; ctx.lineJoin = "round"; ctx.lineCap = "round"; ctx.stroke();
      ctx.beginPath(); ctx.arc(X(n - 1), Y(pts[n - 1]!), 2.4, 0, 7); ctx.fillStyle = color; ctx.fill();
      if (p < 1) raf = requestAnimationFrame(draw);
    };
    to = window.setTimeout(() => { raf = requestAnimationFrame(draw); }, delay);
    return () => { clearTimeout(to); cancelAnimationFrame(raf); };
  }, [points, color, delay, dur]);
  return <canvas ref={ref} />;
}

/* ── hero net-worth chart ──────────────────────────────── */
function HeroChart({ delay, dur }: { delay: number; dur: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth, h = cv.clientHeight;
    cv.width = w * dpr; cv.height = h * dpr; ctx.scale(dpr, dpr);
    // A simulated net-worth story: a steady climb interrupted by one small dip
    // and one large drawdown (a market correction), then a strong recovery.
    const raw = [104, 108, 112, 115, 118, 114, 118, 123, 128, 132, 134, 126, 118, 122, 128, 133, 137.5];
    const pts = smoothCurve(raw, 260);
    const min = Math.min(...pts) - 1.5, max = Math.max(...pts) + 1.5;
    const X = (i: number) => (i / (pts.length - 1)) * w;
    const Y = (v: number) => h - 4 - ((v - min) / (max - min)) * (h - 10);
    let raf = 0, to = 0, start = 0;
    const draw = (now: number) => {
      if (!start) start = now;
      const p = Math.min((now - start) / dur, 1);
      const n = Math.max(2, Math.floor(easeOut(p) * pts.length));
      ctx.clearRect(0, 0, w, h);
      const g = ctx.createLinearGradient(0, 0, 0, h);
      g.addColorStop(0, "rgba(201,249,80,0.28)"); g.addColorStop(1, "rgba(201,249,80,0)");
      ctx.save();
      ctx.beginPath(); ctx.moveTo(X(0), Y(pts[0]!));
      for (let i = 1; i < n; i++) ctx.lineTo(X(i), Y(pts[i]!));
      ctx.lineTo(X(n - 1), h); ctx.lineTo(0, h); ctx.closePath();
      ctx.fillStyle = g; ctx.fill();
      ctx.restore();
      ctx.beginPath(); ctx.moveTo(X(0), Y(pts[0]!));
      for (let i = 1; i < n; i++) ctx.lineTo(X(i), Y(pts[i]!));
      ctx.strokeStyle = "#C9F950"; ctx.lineWidth = 2.4; ctx.lineJoin = "round"; ctx.lineCap = "round";
      ctx.shadowColor = "rgba(201,249,80,0.6)"; ctx.shadowBlur = 12; ctx.stroke();
      ctx.shadowBlur = 0;
      const tx = X(n - 1), ty = Y(pts[n - 1]!);
      ctx.beginPath(); ctx.arc(tx, ty, 3.4, 0, 7); ctx.fillStyle = "#C9F950"; ctx.fill();
      ctx.beginPath(); ctx.arc(tx, ty, 6, 0, 7); ctx.strokeStyle = "rgba(201,249,80,0.4)"; ctx.lineWidth = 1.5; ctx.stroke();
      if (p < 1) raf = requestAnimationFrame(draw);
    };
    to = window.setTimeout(() => { raf = requestAnimationFrame(draw); }, delay);
    return () => { clearTimeout(to); cancelAnimationFrame(raf); };
  }, [delay, dur]);
  return <canvas ref={ref} />;
}

/* ── mesh + constellation background ───────────────────── */
function useShowcaseBg(
  meshRef: React.RefObject<HTMLCanvasElement | null>,
  netRef: React.RefObject<HTMLCanvasElement | null>,
  hostRef: React.RefObject<HTMLDivElement | null>,
) {
  useEffect(() => {
    const host = hostRef.current, mesh = meshRef.current, net = netRef.current;
    if (!host || !mesh || !net) return;
    const mctx = mesh.getContext("2d"), nctx = net.getContext("2d");
    if (!mctx || !nctx) return;
    let raf = 0, W = 0, H = 0;
    const dpr = Math.min(window.devicePixelRatio || 1, 3);
    const blobs = [
      { x: .7, y: .2, r: .5, c: [201, 249, 80], vx: .00006, vy: .00008 },
      { x: .25, y: .8, r: .55, c: [96, 165, 250], vx: -.00007, vy: -.00005 },
      { x: .55, y: .6, r: .42, c: [167, 139, 250], vx: .00005, vy: -.00007 },
    ];
    const N = 46;
    const parts = Array.from({ length: N }, () => ({
      x: Math.random(), y: Math.random(),
      vx: (Math.random() - .5) * .00022, vy: (Math.random() - .5) * .00022,
      r: Math.random() * 1.4 + .6,
    }));
    const resize = () => {
      W = host.clientWidth; H = host.clientHeight;
      for (const c of [mesh, net]) { c.width = W * dpr; c.height = H * dpr; }
      mctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      nctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    const ro = new ResizeObserver(resize); ro.observe(host);
    let t = 0;
    const frame = () => {
      t++;
      mctx.clearRect(0, 0, W, H);
      mctx.globalCompositeOperation = "lighter";
      for (const b of blobs) {
        b.x += b.vx; b.y += b.vy;
        if (b.x < .1 || b.x > .9) b.vx *= -1;
        if (b.y < .1 || b.y > .9) b.vy *= -1;
        const cx = b.x * W, cy = b.y * H, rad = b.r * Math.min(W, H);
        const g = mctx.createRadialGradient(cx, cy, 0, cx, cy, rad);
        g.addColorStop(0, `rgba(${b.c[0]},${b.c[1]},${b.c[2]},0.12)`);
        g.addColorStop(1, `rgba(${b.c[0]},${b.c[1]},${b.c[2]},0)`);
        mctx.fillStyle = g; mctx.beginPath(); mctx.arc(cx, cy, rad, 0, 7); mctx.fill();
      }
      mctx.globalCompositeOperation = "source-over";
      const flow = t * 0.0016;
      for (const p of parts) {
        p.vx += Math.cos((p.y * 6) + flow) * 0.0000018;
        p.vy += Math.sin((p.x * 6) + flow) * 0.0000018;
        p.x += p.vx; p.y += p.vy;
        if (p.x < 0) p.x += 1; if (p.x > 1) p.x -= 1;
        if (p.y < 0) p.y += 1; if (p.y > 1) p.y -= 1;
        p.vx *= .992; p.vy *= .992;
      }
      const px = (p: typeof parts[number]) => p.x * W, py = (p: typeof parts[number]) => p.y * H;
      nctx.clearRect(0, 0, W, H);
      for (let i = 0; i < N; i++) {
        for (let j = i + 1; j < N; j++) {
          const a = parts[i]!, b = parts[j]!;
          const dx = (a.x - b.x) * W, dy = (a.y - b.y) * H, d = Math.hypot(dx, dy);
          if (d < 132) {
            nctx.strokeStyle = `rgba(201,249,80,${(1 - d / 132) * 0.42})`;
            nctx.lineWidth = 1;
            nctx.beginPath(); nctx.moveTo(px(a), py(a)); nctx.lineTo(px(b), py(b)); nctx.stroke();
          }
        }
      }
      for (const p of parts) {
        const tw = 0.55 + Math.sin(t * 0.03 + p.x * 40) * 0.3;
        nctx.beginPath(); nctx.arc(px(p), py(p), p.r, 0, 7);
        nctx.fillStyle = `rgba(230,245,190,${tw})`; nctx.fill();
      }
      raf = requestAnimationFrame(frame);
    };
    frame();
    return () => { cancelAnimationFrame(raf); ro.disconnect(); };
  }, [meshRef, netRef, hostRef]);
}

/* ── 3D parallax showcase ──────────────────────────────── */
const CAPS = [
  { h: "One clear view of every account.", p: "Checking, savings, cards, investments and manual assets — reconciled nightly." },
  { h: "An agent that watches the small stuff.", p: "Price hikes, forgotten trials and anomalies surface before they cost you." },
  { h: "Plans that bend to real life.", p: "Model a sabbatical, a raise or a new car and see every goal shift instantly." },
];

function Showcase() {
  const host = useRef<HTMLDivElement>(null);
  const mesh = useRef<HTMLCanvasElement>(null);
  const net = useRef<HTMLCanvasElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  useShowcaseBg(mesh, net, host);

  useEffect(() => {
    const h = host.current, s = stage.current;
    if (!h || !s) return;
    let tmx = 0, tmy = 0, mx = 0, my = 0, gx = 0, gy = 0, raf = 0;
    const t0 = performance.now();
    const onMove = (e: MouseEvent) => {
      const r = h.getBoundingClientRect();
      tmx = ((e.clientX - r.left) / r.width - .5) * 2;
      tmy = ((e.clientY - r.top) / r.height - .5) * 2;
    };
    const onLeave = () => { tmx = 0; tmy = 0; };
    const onTilt = (e: DeviceOrientationEvent) => {
      if (e.gamma == null || e.beta == null) return;
      gx = Math.max(-1, Math.min(1, e.gamma / 22));
      gy = Math.max(-1, Math.min(1, (e.beta - 45) / 22));
    };
    h.addEventListener("mousemove", onMove);
    h.addEventListener("mouseleave", onLeave);
    window.addEventListener("deviceorientation", onTilt);
    const loop = (now: number) => {
      const t = (now - t0) / 1000;
      const ax = Math.sin(t * 0.34) * 0.6 + Math.sin(t * 0.11) * 0.28;
      const ay = Math.cos(t * 0.27) * 0.5 + Math.sin(t * 0.17) * 0.22;
      mx += ((tmx + gx) - mx) * 0.05;
      my += ((tmy + gy) - my) * 0.05;
      const rotY = (ax + mx * 1.15) * 5.5;
      const rotX = -(ay + my * 1.15) * 5.5;
      const tz = Math.sin(t * 0.5) * 8;
      s.style.transform = `translateZ(${tz}px) rotateY(${rotY}deg) rotateX(${rotX}deg)`;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => {
      cancelAnimationFrame(raf);
      h.removeEventListener("mousemove", onMove);
      h.removeEventListener("mouseleave", onLeave);
      window.removeEventListener("deviceorientation", onTilt);
    };
  }, []);

  const nw = useCountUp(137515, REVEAL.hero.dur, REVEAL.hero.delay);
  const c1 = useCountUp(14820, REVEAL.checking.dur, REVEAL.checking.delay);
  const c2 = useCountUp(28640, REVEAL.house.dur, REVEAL.house.delay);
  const [cap, setCap] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => setCap((c) => (c + 1) % CAPS.length), 4200);
    return () => clearInterval(id);
  }, []);
  const active = CAPS[cap]!;

  return (
    <div className="showcase" ref={host} aria-hidden="true">
      <canvas className="bg" ref={mesh} />
      <canvas className="bg net" ref={net} />
      <div className="showcase-vignette" />
      <div className="stage">
        <div className="stage-inner" ref={stage}>
          <div className="gcard card-hero float">
            <div className="lbl"><span className="d" /> Net worth · joint</div>
            <div className="big"><span className="cur">$</span>{money(nw)}</div>
            <div className="delta">{Ico.arrow({ style: { width: 13, height: 13, transform: "rotate(-45deg)" } })} +$13,415 this month</div>
            <HeroChart delay={REVEAL.hero.delay} dur={REVEAL.hero.dur} />
          </div>
          <div className="gcard card-mini card-a float">
            <div className="top">
              <div className="logo" style={{ background: "#C9F950", color: "#0A0F02" }}>Mc</div>
              <div><div className="nm">Joint Checking</div><div className="sub">MERCURY ·· 4421</div></div>
            </div>
            <div className="amt">${money(c1)}</div>
            <Sparkline points={JOINT_CHECKING} color="#C9F950" delay={REVEAL.checking.delay} dur={REVEAL.checking.dur} />
          </div>
          <div className="gcard card-mini card-b float">
            <div className="top">
              <div className="logo" style={{ background: "#34D399", color: "#04130C" }}>Wf</div>
              <div><div className="nm">House Fund</div><div className="sub">WEALTHFRONT ·· 9087</div></div>
            </div>
            <div className="amt">${money(c2)}</div>
            <Sparkline points={HOUSE_FUND} color="#34D399" delay={REVEAL.house.delay} dur={REVEAL.house.dur} />
          </div>
          <div className="gcard card-insight float">
            <div className="spark">{Ico.spark()}</div>
            <div>
              <div className="txt"><b>Utilities</b> ran 2.1× your 12-month average this cycle — worth a glance.</div>
              <div className="meta">AGENT · 3 HOURS AGO</div>
            </div>
          </div>
          <div className="gcard card-pill float">
            <div className="avset">
              <div className="av" style={{ background: "#C9F950", color: "#0A0F02" }}>M</div>
              <div className="av" style={{ background: "#60A5FA" }}>A</div>
            </div>
            <div className="lab">Shared with <b>Mira &amp; Adam</b></div>
          </div>
        </div>
      </div>
      <div className="showcase-cap">
        <h2>{active.h}</h2>
        <p>{active.p}</p>
        <div className="showcase-dots">
          {CAPS.map((_, i) => <span key={i} className={i === cap ? "on" : ""} />)}
        </div>
      </div>
    </div>
  );
}

/* ── shell: brand + centred body + footer + showcase ───── */
export function AuthShell({
  eyebrow,
  title,
  subtitle,
  children,
}: {
  eyebrow: string;
  title: ReactNode;
  subtitle: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="fs-auth">
      <div className="panel">
        <div className="brand">
          <div className="mark" />
          <div className="wm">Fin<b>Sight</b></div>
        </div>
        <div className="panel-body">
          <div className="head-eyebrow"><span className="dot" /> {eyebrow}</div>
          <h1 className="head-title">{title}</h1>
          <p className="head-sub">{subtitle}</p>
          {children}
        </div>
        <div className="panel-foot">
          <span className="lock">{Ico.lock({ style: { width: 13, height: 13 } })} 256-bit encrypted</span>
          <span>Local-first · self-hosted</span>
        </div>
      </div>
      <Showcase />
    </div>
  );
}

/** One-time recovery-key reveal, styled to the auth shell. The key lives only
 *  in props for the render's lifetime — it is never persisted client-side.
 *  Shared by SetupScreen (first-run) and RecoverScreen (post-reset). */
export function RecoveryReveal({ recoveryKey, onContinue }: { recoveryKey: string; onContinue: () => void }) {
  const [confirmed, setConfirmed] = useState(false);
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setCopied(true);
      toast.success("Recovery key copied");
    } catch {
      toast.error("Could not copy — select and copy the key manually");
    }
  };
  return (
    <div className="rk">
      <div className="rk-key">{recoveryKey}</div>
      <button type="button" className="btn-ghost" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy to clipboard"}
      </button>
      <label className="rk-check">
        <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} aria-label="I saved my recovery key" />
        <span>I saved my recovery key</span>
      </label>
      <button className="submit" type="button" style={{ marginTop: 18 }} disabled={!confirmed} onClick={onContinue}>
        Continue <span className="arw">{Ico.arrow()}</span>
      </button>
    </div>
  );
}
