#!/usr/bin/env bash
# Build the desktop UI and refuse a bundle that carries React twice.
#
# The bundler (rolldown-vite) intermittently emits a duplicate React copy
# (~712 KB instead of ~560 KB, two `e.useState=function` shims). That
# bundle renders a blank window - the app dies with "Cannot read
# properties of null (reading 'useState')" - so never ship without this
# check. Retries the build until the bundle is clean.
set -u
cd "$(dirname "$0")/.." || exit 1

check() {
  python - "$1" <<'PY'
import os, sys
d = sys.argv[1]
f = [x for x in os.listdir(d) if x.endswith('.js') and x.startswith('index-')]
if len(f) != 1:
    print('BAD: expected one index-*.js, found', f)
    sys.exit(1)
js = open(os.path.join(d, f[0]), encoding='utf8', errors='replace').read()
n = js.count('e.useState=function')
print(f'{f[0]} shims={n} size={len(js)}')
sys.exit(0 if n == 1 else 1)
PY
}

cd desktop/ui-next || exit 1
for i in 1 2 3 4; do
  rm -rf dist
  npm run build -- --base=./ >/dev/null 2>&1
  if check dist/assets; then
    echo "BUILD_CLEAN (attempt $i)"
    exit 0
  fi
  echo "retrying: duplicate React in the bundle (attempt $i)"
done
echo "BUILD_BROKEN: the bundler keeps duplicating React"
exit 1
