import io, json, re

# version 0.4.1 for the test build (pre-release; the updater stays on v0.3.8)
P = r"C:\Tools\QuotaCards\desktop\src-tauri\tauri.conf.json"
raw = io.open(P, encoding="utf-8", newline="").read()
assert '"version": "0.4.0"' in raw
raw = raw.replace('"version": "0.4.0"', '"version": "0.4.1"', 1)
io.open(P, "w", encoding="utf-8", newline="").write(raw)
print("version 0.4.1")
