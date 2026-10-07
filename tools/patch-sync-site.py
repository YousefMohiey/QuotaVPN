import io

P = r"C:\Tools\QuotaCards\tools\sync-site.sh"
t = io.open(P, encoding="utf-8", newline="").read()
old = '''cd "$ROOT/desktop/ui-next"
rm -rf dist
# The project's own script, not a bare vite build: the tsc pass it runs first
# is load bearing (a raw vite build ships a bundle whose React dispatcher is
# null and the page mounts to nothing).
npm run build -- --base=./ >/dev/null
cd "$ROOT"
'''
new = '''# The guarded build, never a bare vite build: rolldown-vite intermittently
# emits a bundle carrying React twice (two e.useState shims), which mounts to
# nothing. build-ui.sh checks for that shim and retries until clean.
bash "$ROOT/tools/build-ui.sh" >/dev/null
cd "$ROOT"
'''
assert t.count(old) == 1, "count %d" % t.count(old)
io.open(P, "w", encoding="utf-8", newline="").write(t.replace(old, new, 1))
print("sync-site patched")
