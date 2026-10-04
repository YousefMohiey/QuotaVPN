import json, os

d = 'dist/assets'
f = [x for x in os.listdir(d) if x.endswith('.js') and x.startswith('index-')][0]
mp = os.path.join(d, f + '.map')
data = json.load(open(mp, encoding='utf8'))
srcs = [s.replace(chr(92), '/') for s in data.get('sources', [])]

print('total sources:', len(srcs))
print()
print('=== core react module entries:')
for s in srcs:
    if s.endswith('node_modules/react/index.js') or 'node_modules/react/cjs/' in s:
        print('  ', s)
print()
print('=== react-dom core entries:')
for s in srcs:
    if 'node_modules/react-dom/cjs/' in s or s.endswith('node_modules/react-dom/index.js'):
        print('  ', s)
print()
print('=== any source with a second react copy (path hints):')
for s in srcs:
    if 'react' in s.lower() and ('copy' in s.lower() or '.pnpm' in s or 'vendor' in s.lower()):
        print('  ', s)
