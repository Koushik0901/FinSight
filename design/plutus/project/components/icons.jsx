/* Monoline icon set — 16px, stroke 1.4, currentColor */
const I = (() => {
  const s = (children, viewBox = "0 0 16 16") => (props) => (
    <svg viewBox={viewBox} width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...props}>{children}</svg>
  );

  const Today      = s(<><circle cx="8" cy="8" r="5.5" /><path d="M8 4.5v3.5l2.2 1.4" /></>);
  const Wallet     = s(<><rect x="2" y="4" width="12" height="9" rx="1.5" /><path d="M2 7h12" /><circle cx="11.5" cy="10" r="0.6" fill="currentColor" stroke="none" /></>);
  const Flow       = s(<><path d="M2 11.5 5 8l2.5 2.5L13 4" /><path d="M9.5 4H13v3.5" /></>);
  const Grid       = s(<><rect x="2" y="2" width="5" height="5" rx="0.6" /><rect x="9" y="2" width="5" height="5" rx="0.6" /><rect x="2" y="9" width="5" height="5" rx="0.6" /><rect x="9" y="9" width="5" height="5" rx="0.6" /></>);
  const Repeat     = s(<><path d="M3 6.5V5a1.5 1.5 0 0 1 1.5-1.5h7L13 5l-1.5 1.5" /><path d="M13 9.5V11a1.5 1.5 0 0 1-1.5 1.5h-7L3 11l1.5-1.5" /></>);
  const Goal       = s(<><circle cx="8" cy="8" r="6" /><circle cx="8" cy="8" r="3" /><circle cx="8" cy="8" r="0.8" fill="currentColor" stroke="none" /></>);
  const Lego       = s(<><path d="M3 7h10v6H3z" /><path d="M5 7V4.5h2V7M9 7V4.5h2V7" /></>);
  const Gear       = s(<><circle cx="8" cy="8" r="2" /><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.5 3.5l1.4 1.4M11.1 11.1l1.4 1.4M3.5 12.5l1.4-1.4M11.1 4.9l1.4-1.4" /></>);
  const Sparkle    = s(<><path d="M8 2v3M8 11v3M2 8h3M11 8h3M4 4l2 2M10 10l2 2M4 12l2-2M10 6l2-2" /></>);
  const Search     = s(<><circle cx="7" cy="7" r="4.5" /><path d="m10.5 10.5 3 3" /></>);
  const ArrowR     = s(<path d="M3 8h10M9 4l4 4-4 4" />);
  const ArrowL     = s(<path d="M13 8H3M7 4 3 8l4 4" />);
  const ArrowDown  = s(<path d="M8 3v10M4 9l4 4 4-4" />);
  const ArrowUp    = s(<path d="M8 13V3M4 7l4-4 4 4" />);
  const Plus       = s(<path d="M8 3v10M3 8h10" />);
  const Check      = s(<path d="m3 8 3.5 3.5L13 5" />);
  const X          = s(<path d="m4 4 8 8M12 4l-8 8" />);
  const Lock       = s(<><rect x="3" y="7" width="10" height="7" rx="1.5" /><path d="M5 7V5a3 3 0 0 1 6 0v2" /></>);
  const Eye        = s(<><path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" /><circle cx="8" cy="8" r="1.8" /></>);
  const EyeOff     = s(<><path d="M3 3l10 10" /><path d="M6 6.2C3.8 7.4 1.5 8 1.5 8s2.5 4.5 6.5 4.5c1.1 0 2.1-.3 3-.7" /><path d="M9.6 4.1A6.7 6.7 0 0 1 14.5 8s-.7 1.3-2 2.5" /></>);
  const Filter     = s(<path d="M2 4h12L9.5 9v4L6.5 14V9z" />);
  const Bolt       = s(<path d="m9 1.5-6 7.5h4l-1 5.5 6-7.5H8z" />);
  const Bell       = s(<><path d="M4 11V8a4 4 0 0 1 8 0v3l1 1.5H3z" /><path d="M6.5 13a1.5 1.5 0 0 0 3 0" /></>);
  const Cmd        = s(<path d="M5 3.5a1.5 1.5 0 1 1 0 3h6a1.5 1.5 0 1 1 0 3M5 9.5a1.5 1.5 0 1 0 0 3h6a1.5 1.5 0 1 0 0-3M5 6.5h6v3H5z" />);
  const Bank       = s(<><path d="M2 6.5 8 3l6 3.5" /><path d="M3 6.5h10" /><path d="M4 7v5M7 7v5M9 7v5M12 7v5" /><path d="M2.5 13.5h11" /></>);
  const Up         = s(<path d="m4 10 4-4 4 4" />);
  const Down       = s(<path d="m4 6 4 4 4-4" />);
  const Spark      = s(<path d="M2 11l3-4 2.5 2.5L11 5l3 4" />);
  const Calendar   = s(<><rect x="2.5" y="3.5" width="11" height="10" rx="1.2" /><path d="M2.5 6h11M5 2.2v2.6M11 2.2v2.6" /></>);
  const More       = s(<><circle cx="3.5" cy="8" r="1" fill="currentColor" stroke="none" /><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" /><circle cx="12.5" cy="8" r="1" fill="currentColor" stroke="none" /></>);
  const Drag       = s(<><circle cx="6" cy="4" r="0.8" fill="currentColor" stroke="none" /><circle cx="10" cy="4" r="0.8" fill="currentColor" stroke="none" /><circle cx="6" cy="8" r="0.8" fill="currentColor" stroke="none" /><circle cx="10" cy="8" r="0.8" fill="currentColor" stroke="none" /><circle cx="6" cy="12" r="0.8" fill="currentColor" stroke="none" /><circle cx="10" cy="12" r="0.8" fill="currentColor" stroke="none" /></>);
  const HousE      = s(<path d="M2.5 8 8 3l5.5 5M4 7.5V13h8V7.5" />);
  const Cart       = s(<><path d="M2.5 3h2l1 8h7M5.5 8h6.5" /><circle cx="6.5" cy="13" r="0.9" /><circle cx="11" cy="13" r="0.9" /></>);
  const Fork       = s(<><path d="M4 2v12M4 6h3a1 1 0 0 1 1 1v4" /><path d="M12 2v12M12 6V2" /></>);
  const Car        = s(<><path d="M2.5 11V8l1.5-3h8l1.5 3v3" /><path d="M2.5 11h11M3.5 13a1 1 0 1 0 0-2M12.5 13a1 1 0 1 0 0-2" /></>);
  const Bulb       = s(<><path d="M5.5 10.5a4 4 0 1 1 5 0V12H5.5z" /><path d="M6.5 14h3" /></>);
  const Box        = s(<><path d="M2.5 5.5 8 3l5.5 2.5L8 8z" /><path d="M2.5 5.5V11L8 13.5V8M13.5 5.5V11L8 13.5" /></>);
  const Heart      = s(<path d="M8 13s-5-3.2-5-7a2.5 2.5 0 0 1 5-.5 2.5 2.5 0 0 1 5 .5c0 3.8-5 7-5 7z" />);
  const Tag        = s(<><path d="M3 3h5.5L13 7.5 8.5 12 4 7.5z" /><circle cx="6" cy="6" r="0.8" fill="currentColor" stroke="none" /></>);
  const Plane      = s(<path d="m2 9 12-5-3 11-3-4.5z" />);
  const Gift       = s(<><rect x="2.5" y="6" width="11" height="3" rx="0.6" /><rect x="3.5" y="9" width="9" height="5" rx="0.6" /><path d="M8 6v8M5.5 6c0-1.5 1-2.5 2.5-2.5S10.5 4.5 10.5 6" /></>);
  const Pencil     = s(<><path d="m3 13 1-3 7-7 2 2-7 7z" /><path d="m9 5 2 2" /></>);
  const Refresh    = s(<><path d="M13 3v3.5h-3.5" /><path d="M3 13v-3.5h3.5" /><path d="M12.5 7A5 5 0 0 0 4 5.5M3.5 9A5 5 0 0 0 12 10.5" /></>);
  const Trash      = s(<><path d="M3 4.5h10M6 4.5V3a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5" /><path d="M4.5 4.5 5 13.2c0 .4.4.8.8.8h4.4c.4 0 .8-.4.8-.8L11.5 4.5" /><path d="M7 7v5M9 7v5" /></>);
  const Maximize   = s(<><path d="M3 6V3h3M13 6V3h-3M3 10v3h3M13 10v3h-3" /></>);
  const Copy       = s(<><rect x="5" y="5" width="8" height="8" rx="1.2" /><path d="M11 5V4a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h1" /></>);
  const Donut      = s(<><circle cx="8" cy="8" r="5.5" /><circle cx="8" cy="8" r="2.5" /></>);
  const Bar        = s(<><path d="M3 13V8M6.5 13V4M10 13v-7M13.5 13v-3" /></>);
  const Line       = s(<><path d="M2 11 5 7l3 2 5-6" /><circle cx="13" cy="3" r="0.8" fill="currentColor" stroke="none" /></>);
  const TextIco    = s(<><path d="M3.5 4.5h9M8 4.5V13M5.5 13h5" /></>);
  const Building   = s(<><rect x="3" y="3" width="10" height="11" rx="0.8" /><path d="M3 7h10M3 11h10M6 3v11M10 3v11" /></>);
  const Target     = s(<><circle cx="8" cy="8" r="6" /><circle cx="8" cy="8" r="3.5" /><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" /></>);
  const Layers     = s(<><path d="M8 2 2 5l6 3 6-3z" /><path d="m2 8 6 3 6-3M2 11l6 3 6-3" /></>);
  const Activity   = s(<path d="M1.5 8h2.5l1.5-4 3 8 1.5-4h4.5" />);
  const ListI      = s(<><path d="M5 4h9M5 8h9M5 12h9" /><circle cx="2.5" cy="4" r="0.8" fill="currentColor" stroke="none" /><circle cx="2.5" cy="8" r="0.8" fill="currentColor" stroke="none" /><circle cx="2.5" cy="12" r="0.8" fill="currentColor" stroke="none" /></>);
  const Pin        = s(<><path d="M10 2 14 6l-2.5 1L9 9.5l-1 5-3.5-3.5 5-1L11.5 7l-3-3z" /></>);

  // category icons by id
  const catIcon = {
    housing: HousE, groceries: Cart, dining: Fork, transport: Car, utilities: Bulb,
    subs: Box, health: Heart, shopping: Tag, travel: Plane, gifts: Gift, income: ArrowDown,
  };

  return {
    Today, Wallet, Flow, Grid, Repeat, Goal, Lego, Gear, Sparkle, Search,
    ArrowR, ArrowL, ArrowUp, ArrowDown, Plus, Check, X, Lock, Eye, EyeOff,
    Filter, Bolt, Bell, Cmd, Bank, Up, Down, Spark, Calendar, More, Drag,
    Pencil, Refresh, Trash, Maximize, Copy, Donut, Bar, Line, TextIco,
    Building, Target, Layers, Activity, ListI, Pin,
    catIcon,
  };
})();
window.I = I;
