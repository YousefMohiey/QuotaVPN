import { useEffect, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { Info, X } from "lucide-react"
import { useApp } from "@/state/app"
import { useI18n } from "@/lib/i18n"

const EASE_OUT = [0.1, 0.9, 0.2, 1] as const

/** The transport warning as a notification: it used to sit inline in the
    setup card and push the column around. It floats top right now and the
    user can close it; a new transport choice arms it again. */
export function QuotaToast() {
  const { transport } = useApp()
  const { t } = useI18n()
  const [closed, setClosed] = useState(false)
  useEffect(() => {
    setClosed(false)
  }, [transport])
  return (
    <AnimatePresence>
      {transport !== "vless" && !closed && (
        <motion.div
          role="status"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, y: 16 }}
          transition={{ duration: 0.28, ease: EASE_OUT }}
          className="fixed bottom-6 right-6 z-[60] flex w-[320px] max-w-[calc(100vw-3rem)] items-start gap-2.5 rounded-[12px] border border-[var(--brand-line)] bg-[#10151b]/95 px-3 py-2.5 text-[12px] leading-[1.5] text-txt2 shadow-[0_18px_50px_rgb(0_0_0/0.5)] backdrop-blur-[24px]"
        >
          <Info className="mt-px size-4 shrink-0 text-brand-strong" aria-hidden />
          <span className="min-w-0 flex-1">{t("quotaWarn")}</span>
          <button
            type="button"
            onClick={() => setClosed(true)}
            aria-label={t("dismiss")}
            className="-me-1 -mt-0.5 grid size-6 shrink-0 place-items-center rounded-[8px] transition-colors hover:bg-white/[0.06]"
          >
            <X className="size-3.5" aria-hidden />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
