/** The in-app Q mark. The ring follows the text colour and the tail follows
    the active theme accent, so the logo changes with the preset. The Windows
    exe/tray icon stays the static asset; this is the mark on the rail. */
export function QuotaMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 48 48" className={className} aria-hidden focusable="false">
      <rect width="48" height="48" rx="13" fill="var(--surface-3)" />
      <circle cx="23.5" cy="22.5" r="10.8" fill="none" stroke="var(--txt)" strokeWidth="5.4" />
      <path
        d="M28.4 30.2 37.6 39"
        fill="none"
        stroke="var(--brand)"
        strokeWidth="5.4"
        strokeLinecap="round"
      />
    </svg>
  )
}
