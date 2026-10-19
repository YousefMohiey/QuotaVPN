/** The product mark: a ring partly spent, which is what this app is about. */
export function QuotaMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden>
      <circle cx="16" cy="16" r="14" fill="rgb(44 95 216 / 0.10)" stroke="rgb(44 95 216 / 0.40)" strokeWidth="1" />
      <circle cx="16" cy="16" r="10.5" fill="none" stroke="rgb(255 255 255 / 0.10)" strokeWidth="3" />
      <circle
        cx="16"
        cy="16"
        r="10.5"
        fill="none"
        stroke="#3d7bff"
        strokeWidth="3"
        strokeLinecap="round"
        strokeDasharray="41 66"
        transform="rotate(-90 16 16)"
      />
      <circle cx="16" cy="16" r="2.4" fill="#7fa6ff" />
    </svg>
  )
}
