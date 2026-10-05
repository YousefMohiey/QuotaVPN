import io
src = io.open(r"C:\Tools\QuotaCards\tools\ph-theme-1.py", encoding="utf-8").read()
i = src.index("# ---- the header")
css = src[i:]
css = css.replace("sub4('''.hero {''',", "sub4('''/* Hero: status up top, one switch below. Compact: Home never scrolls. */''',")
head = 'import io\nP = r"C:\\Tools\\QuotaCards\\android\\tauri-app\\ui\\style.css"\nt = io.open(P, encoding="utf-8", newline="").read().replace("\\r\\n", "\\n")\ndef sub4(old, new, cnt=1):\n    global t\n    assert t.count(old) == cnt, "COUNT %d != %d FOR %r" % (t.count(old), cnt, old[:90])\n    t = t.replace(old, new, cnt)\n'
tail = '\nio.open(P, "w", encoding="utf-8", newline="").write(t.replace("\\n", "\\r\\n"))\nprint("css ok")\n'
io.open(r"C:\Tools\QuotaCards\tools\ph-theme-css.py", "w", encoding="utf-8").write(head + css + tail)
print("runner written")
