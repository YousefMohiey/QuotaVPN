// One stack for everything the back action unwinds.
//
// Pages are a trail the shell owns (App.tsx), and anything that opens on top of
// a page, a dialog, a picker, a confirm, registers a closer here. Back closes
// the topmost thing first, then walks the trail, which is what a person expects
// from every back control in the app.

type Closer = () => void

const closers: Closer[] = []

/** Register a closer for an overlay. Returns the unregister function. */
export function pushCloser(closer: Closer): () => void {
  closers.push(closer)
  return () => {
    const i = closers.indexOf(closer)
    if (i >= 0) closers.splice(i, 1)
  }
}

/** Close the topmost overlay. False when there is nothing open. */
export function closeTopOverlay(): boolean {
  const top = closers[closers.length - 1]
  if (!top) return false
  closers.pop()
  top()
  return true
}

/** How many overlays are open, for tests and for the shell to know. */
export function overlayDepth(): number {
  return closers.length
}
