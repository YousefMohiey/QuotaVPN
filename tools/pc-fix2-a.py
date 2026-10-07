import io, json

ROOT = r"C:\Tools\QuotaCards"

def load(p):
    return io.open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def save(p, t):
    nl = "\r\n" if b"\r\n" in io.open(p, "rb").read()[:4000] else "\n"
    io.open(p, "w", encoding="utf-8", newline="").write(t.replace("\n", nl))

def sub(t, old, new, n=1):
    assert t.count(old) == n, "COUNT %d FOR %r" % (t.count(old), old[:80])
    return t.replace(old, new, n)

# ---- version back to 0.4.0 (the owner never asked for 0.4.1) ----
P = ROOT + r"\desktop\src-tauri\tauri.conf.json"
t = load(P)
t = sub(t, '"version": "0.4.1"', '"version": "0.4.0"')
save(P, t)
print("version back to 0.4.0")

# ---- EN strings ----
P = ROOT + r"\desktop\ui-next\src\lib\i18n\en.ts"
t = load(P)
t = sub(t, '  "srvLocation": "Server location",\n',
        '  "srvLocation": "Server location",\n  "yourLocation": "Your location",\n')
t = sub(t, '  "appsHint": "Changes apply next time you connect.",\n',
        '  "appsHint": "Changes apply next time you connect.",\n'
        '  "appsBody": "Choose which applications use the QuotaVPN connection.",\n'
        '  "appsModeLbl": "Routing mode",\n'
        '  "appsListTitle": "Applications",\n')
t = sub(t, '  "appsSummary": "{s} selected \u00b7 {t} apps",',
        '  "appsSummary": "{t} applications \u00b7 {s} selected",')
t = sub(t, '  "appsSummaryMatch": "{s} selected \u00b7 {t} apps \u00b7 {m} match {q}",',
        '  "appsSummaryMatch": "{t} applications \u00b7 {s} selected \u00b7 {m} match {q}",')
t = sub(t, '  "setupBody": "Choose a preset or configure your connection.",\n',
        '  "setupBody": "Choose a preset or configure your connection.",\n'
        '  "presetHeading": "Connection preset",\n'
        '  "configHeading": "Configuration",\n'
        '  "configBody": "Adjust how QuotaVPN connects.",\n'
        '  "tagGaming": "Gaming",\n'
        '  "tagLowLatency": "Lower latency",\n'
        '  "tagEaGames": "EA games",\n'
        '  "tagStreaming": "Streaming",\n'
        '  "tagVideoPlatforms": "Video platforms",\n'
        '  "tagHighStability": "Higher stability",\n')
save(P, t)
print("en.ts ok")

# ---- AR strings ----
P = ROOT + r"\desktop\ui-next\src\lib\i18n\ar.ts"
t = load(P)
t = sub(t, '  "srvLocation": "\u0645\u0648\u0642\u0639 \u0627\u0644\u062e\u0627\u062f\u0645",\n',
        '  "srvLocation": "\u0645\u0648\u0642\u0639 \u0627\u0644\u062e\u0627\u062f\u0645",\n  "yourLocation": "\u0645\u0648\u0642\u0639\u0643",\n')
t = sub(t, '  "appsHint": "\u0633\u064a\u062a\u0645 \u062a\u0637\u0628\u064a\u0642 \u0627\u0644\u062a\u063a\u064a\u064a\u0631\u0627\u062a \u0639\u0646\u062f \u0627\u0644\u0627\u062a\u0635\u0627\u0644 \u0627\u0644\u062a\u0627\u0644\u064a.",\n',
        '  "appsHint": "\u0633\u064a\u062a\u0645 \u062a\u0637\u0628\u064a\u0642 \u0627\u0644\u062a\u063a\u064a\u064a\u0631\u0627\u062a \u0639\u0646\u062f \u0627\u0644\u0627\u062a\u0635\u0627\u0644 \u0627\u0644\u062a\u0627\u0644\u064a.",\n'
        '  "appsBody": "\u0627\u062e\u062a\u0631 \u0627\u0644\u062a\u0637\u0628\u064a\u0642\u0627\u062a \u0627\u0644\u062a\u064a \u062a\u0633\u062a\u062e\u062f\u0645 \u0627\u062a\u0635\u0627\u0644 QuotaVPN.",\n'
        '  "appsModeLbl": "\u0648\u0636\u0639 \u0627\u0644\u062a\u0648\u062c\u064a\u0647",\n'
        '  "appsListTitle": "\u0627\u0644\u062a\u0637\u0628\u064a\u0642\u0627\u062a",\n')
t = sub(t, '  "appsSummary": "{s} \u0645\u062d\u062f\u062f \u00b7 {t} \u062a\u0637\u0628\u064a\u0642",',
        '  "appsSummary": "{t} \u062a\u0637\u0628\u064a\u0642\u0627\u062a \u00b7 {s} \u0645\u062d\u062f\u062f",')
t = sub(t, '  "appsSummaryMatch": "{s} \u0645\u062d\u062f\u062f \u00b7 {t} \u062a\u0637\u0628\u064a\u0642 \u00b7 {m} \u0646\u062a\u064a\u062c\u0629 \u0639\u0646 {q}",',
        '  "appsSummaryMatch": "{t} \u062a\u0637\u0628\u064a\u0642\u0627\u062a \u00b7 {s} \u0645\u062d\u062f\u062f \u00b7 {m} \u0646\u062a\u064a\u062c\u0629 \u0639\u0646 {q}",')
t = sub(t, '  "setupBody": "\u0627\u062e\u062a\u0631 \u062d\u0632\u0645\u0629 \u0623\u0648 \u0627\u0636\u0628\u0637 \u0627\u062a\u0635\u0627\u0644\u0643.",\n',
        '  "setupBody": "\u0627\u062e\u062a\u0631 \u062d\u0632\u0645\u0629 \u0623\u0648 \u0627\u0636\u0628\u0637 \u0627\u062a\u0635\u0627\u0644\u0643.",\n'
        '  "presetHeading": "\u0627\u0644\u0625\u0639\u062f\u0627\u062f \u0627\u0644\u0645\u0633\u0628\u0642",\n'
        '  "configHeading": "\u0627\u0644\u062a\u0643\u0648\u064a\u0646",\n'
        '  "configBody": "\u0627\u0636\u0628\u0637 \u0637\u0631\u064a\u0642\u0629 \u0627\u062a\u0635\u0627\u0644 QuotaVPN.",\n'
        '  "tagGaming": "\u0623\u0644\u0639\u0627\u0628",\n'
        '  "tagLowLatency": "\u0632\u0645\u0646 \u0627\u0633\u062a\u062c\u0627\u0628\u0629 \u0623\u0642\u0644",\n'
        '  "tagEaGames": "\u0623\u0644\u0639\u0627\u0628 EA",\n'
        '  "tagStreaming": "\u0628\u062b",\n'
        '  "tagVideoPlatforms": "\u0645\u0646\u0635\u0627\u062a \u0627\u0644\u0641\u064a\u062f\u064a\u0648",\n'
        '  "tagHighStability": "\u0627\u0633\u062a\u0642\u0631\u0627\u0631 \u0623\u0639\u0644\u0649",\n')
save(P, t)
print("ar.ts ok")
