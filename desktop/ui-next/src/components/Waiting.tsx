import { useId } from "react"
import { motion } from "motion/react"
import { cn } from "@/lib/utils"

/**
 * The waiting state while the tunnel is negotiated.
 *
 * One long arc sweeps clockwise, a shorter and thinner one counter-rotates
 * under it, and a soft glow breathes from the middle: three speeds, so the
 * shape reads as working rather than as a decoration that happens to spin.
 * Under reduced motion nothing travels; the arc stands still and the glow
 * fades in and out instead.
 */
export function WaitingRing({ calm, className }: { calm?: boolean | null; className?: string }) {
  const still = calm === true
  const gid = useId().replace(/:/g, "")
  return (
    <span aria-hidden className={cn("pointer-events-none absolute inset-0 grid place-items-center", className)}>
      <motion.span
        className="absolute inset-[12%] rounded-full"
        style={{ background: "radial-gradient(circle, rgb(44 95 216 / 0.22), transparent 68%)" }}
        initial={false}
        animate={still ? { opacity: 0.5 } : { opacity: [0.35, 0.85, 0.35] }}
        transition={still ? { duration: 0.2 } : { duration: 1.9, repeat: Infinity, ease: "easeInOut" }}
      />
      <svg viewBox="0 0 100 100" className="size-full overflow-visible">
        <defs>
          <linearGradient id={`wrg-${gid}`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--brand-soft)" stopOpacity="0" />
            <stop offset="55%" stopColor="var(--brand-hi)" stopOpacity="0.75" />
            <stop offset="100%" stopColor="var(--brand-soft)" stopOpacity="1" />
          </linearGradient>
        </defs>
        {/* the track the arcs travel on */}
        <circle cx="50" cy="50" r="49" fill="none" stroke="rgb(255 255 255 / 0.07)" strokeWidth="1" />
        <motion.circle
          cx="50"
          cy="50"
          r="49"
          fill="none"
          stroke={`url(#wrg-${gid})`}
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeDasharray="118 190"
          animate={still ? undefined : { rotate: 360 }}
          transition={{ repeat: Infinity, duration: 1.15, ease: "linear" }}
          style={{ transformBox: "fill-box", transformOrigin: "center", willChange: "transform" }}
        />
        <motion.circle
          cx="50"
          cy="50"
          r="45.5"
          fill="none"
          stroke="var(--brand-hi)"
          strokeOpacity="0.5"
          strokeWidth="1.1"
          strokeLinecap="round"
          strokeDasharray="34 245"
          animate={still ? undefined : { rotate: -360 }}
          transition={{ repeat: Infinity, duration: 2.3, ease: "linear" }}
          style={{ transformBox: "fill-box", transformOrigin: "center", willChange: "transform" }}
        />
      </svg>
    </span>
  )
}

/**
 * The same idea for the Valorant plate, which is an octagon: a conic highlight
 * travels across the clipped shape instead of a ring around it, and the glyph
 * underneath stays where it is.
 */
export function WaitingSweep({ calm, clipPath }: { calm?: boolean | null; clipPath: string }) {
  const still = calm === true
  return (
    <span aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden" style={{ clipPath }}>
      <motion.span
        className="absolute -inset-[30%]"
        style={{
          background:
            "conic-gradient(from 0deg, transparent 0deg, rgb(61 123 255 / 0.34) 42deg, rgb(127 166 255 / 0.5) 58deg, transparent 110deg)",
          transformOrigin: "center",
          willChange: "transform",
        }}
        animate={still ? { opacity: [0.35, 0.75, 0.35] } : { rotate: 360 }}
        transition={
          still
            ? { duration: 2.2, repeat: Infinity, ease: "easeInOut" }
            : { rotate: { repeat: Infinity, duration: 1.7, ease: "linear" } }
        }
      />
    </span>
  )
}
