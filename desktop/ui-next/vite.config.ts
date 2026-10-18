import path from "node:path"
import { fileURLToPath } from "node:url"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vite"

const dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(dirname, "./src") },
    // One React, always: radix and motion both declare their own peer copy,
    // and two copies in the dev graph crash every hook call.
    dedupe: ["react", "react-dom"],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // WebView2 on Windows 10/11 and Android WebView both handle modern output.
    target: "chrome110",
    outDir: "dist",
    emptyOutDir: true,
  },
})
