import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { MotionConfig } from "motion/react"
import { TooltipProvider } from "@/components/ui/tooltip"
import { I18nProvider } from "@/lib/i18n"
import { AppStateProvider } from "@/state/app"
import App from "./App"
import "./index.css"

// Right-click stays inside the app: the browser's own menu (Reload, Save
// as, Inspect) has no place in a shipped product window.
window.addEventListener("contextmenu", (e) => e.preventDefault())

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {/* The springs always run. Windows reports "reduce motion" for a plain
        animation-effects-off preference, and this app was collapsing its
        park glide to an instant jump on machines that do; the owner wants
        the dial to travel, so the OS preference is deliberately not
        consulted here. */}
    <MotionConfig reducedMotion="never">
      <I18nProvider>
        <AppStateProvider>
          <TooltipProvider>
            <App />
          </TooltipProvider>
        </AppStateProvider>
      </I18nProvider>
    </MotionConfig>
  </StrictMode>,
)
