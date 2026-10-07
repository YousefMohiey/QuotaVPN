import { useEffect, useState } from "react"
import { AnimatePresence, motion } from "motion/react"
import { TriangleAlert, X } from "lucide-react"
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
          initial={{ opacity: 0, x: 24 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 24 }}
          transition={{ duration: 0.28, ease: EASE_OUT }}
          className="fixed right-4 top-14 z-[60] flex w-[380px] max-w-[calc(100vw-2rem)] items-start gap-2.5 rounded-[12px] border border-warn-line bg-warn-bg px-3.5 py-3 text-[12.5px] leading-[1.5] text-warn shadow-[0_18px_50px_rgb(0_0_0/0.5)] backdrop-blur-[24px]"
        >
          <TriangleAlert className="mt-px size-4 shrink-0" aria-hidden />
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
