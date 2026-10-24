import { cn } from "@/lib/utils"

/**
 * The waiting state while the tunnel is negotiated.
 *
 * One arc, one speed, no glow. The button at rest is a quiet disc with a
 * hairline edge, so the only thing this adds while it works is a single thin
 * arc travelling along that same edge. Nothing pulses and nothing sweeps. The
 * turn is CSS keyframes on the element itself, because the animation library
 * parked the rotation at its end value and the ring stood still while the app
 * said it was connecting.
 */
export function WaitingRing({ className, stroke = 1.6 }: { className?: string; stroke?: number }) {
  return (
    <span aria-hidden className={cn("pointer-events-none absolute inset-0", className)}>
      <svg viewBox="0 0 100 100" className="size-full">
        <circle cx="50" cy="50" r="49.2" fill="none" stroke="rgb(255 255 255 / 0.06)" strokeWidth="1.4" />
        <circle
          cx="50"
          cy="50"
          r="49.2"
          fill="none"
          style={{ stroke: "var(--brand-vivid)" }}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray="72 237"
          className="origin-center animate-spin [transform-box:fill-box] [animation-duration:1.35s]"
        />
      </svg>
    </span>
  )
}
