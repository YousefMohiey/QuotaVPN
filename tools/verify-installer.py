import sys

path = sys.argv[1]
data = open(path, 'rb').read()
print('exe size:', len(data))

def count(needle: bytes) -> int:
    n = 0
    i = data.find(needle)
    while i != -1:
        n += 1
        i = data.find(needle, i + 1)
    return n

shim = b'e.useState=function'
print('react shim occurrences:', count(shim))

for label, s in [
    ('AR fixed phrase (final build)', 'للجهاز بالكامل'),
    ('AR old phrase (previous build)', 'الجهاز بالكامل'),
    ('bundle name final', 'index-CyhyVlUC.js'),
    ('bundle name previous clean', 'index-Bg0MMS7y.js'),
    ('bundle name broken', 'index-uI55U4y4.js'),
    ('setupTitle en', 'Connection setup'),
]:
    b = s.encode('utf8')
    print(f'{label}: {count(b)}')
