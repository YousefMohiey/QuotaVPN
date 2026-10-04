import io

UI = r"C:\Tools\QuotaCards\android\tauri-app\ui"

# ---------------------------------------------------------------- i18n -----
P = UI + r"\app.js"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %s" % (t.count(old), cnt, old[:80])
    t = t.replace(old, new, cnt)

# Short chip labels: the row's sub line already says the whole sentence.
sub('openVpnSettings: "Open VPN settings",', 'openVpnSettings: "Open",')
sub('updTitle: "Updates", updCheck: "Check for updates", updGet: "Download and install",',
    'updTitle: "Updates", updCheck: "Check", updGet: "Install",')
sub('bgBtnAllow: "Allow background running",', 'bgBtnAllow: "Allow",')
sub('bgBtnStop: "Disallow background running",', 'bgBtnStop: "Disallow",')
sub('bgHint: "Some phones stop the VPN when you swipe the app away. Allow background running so it stays on.",',
    'bgHint: "Keep the VPN on when you swipe the app away.",')
sub('bgHintOn: "Background running is allowed. The VPN stays on when you swipe the app away.",',
    'bgHintOn: "The VPN stays on when you swipe the app away.",')
sub('ksHint: "Kill switch: turn on Always-on VPN in the system settings. If the VPN drops, internet stops instead of leaking.",',
    'ksHint: "Turn on Always-on VPN in system settings so a drop never leaks.",')

sub('openVpnSettings: "افتح إعدادات الـVPN",', 'openVpnSettings: "فتح",')
sub('updTitle: "التحديثات", updCheck: "التحقق من التحديثات", updGet: "تنزيل وتثبيت",',
    'updTitle: "التحديثات", updCheck: "تحقق", updGet: "تثبيت",')
sub('bgBtnAllow: "السماح بالعمل في الخلفية",', 'bgBtnAllow: "اسمح",')
sub('bgBtnStop: "إيقاف العمل في الخلفية",', 'bgBtnStop: "أوقف",')
sub('bgHint: "بعض الهواتف توقف الـVPN عند إغلاق التطبيق. اسمح بالعمل في الخلفية ليبقى يعمل.",',
    'bgHint: "أبقِ الـVPN يعمل عند إغلاق التطبيق.",')
sub('bgHintOn: "تم السماح بالعمل في الخلفية. سيبقى الـVPN يعمل عند إغلاق التطبيق.",',
    'bgHintOn: "الـVPN يبقى يعمل عند إغلاق التطبيق.",')
sub('ksHint: "القفل الكامل: فعّل Always-on VPN من إعدادات النظام، وإذا توقف الـVPN سيتوقف الإنترنت بدلاً من تسرب البيانات.",',
    'ksHint: "فعّل Always-on VPN من إعدادات النظام حتى لا يتسرب الاتصال عند الانقطاع.",')

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("i18n ok")

# ---------------------------------------------------------------- html -----
P = UI + r"\index.html"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

# Home: the story line goes; the cards speak for themselves.
sub('''      <h2 data-i18n="setupTitle">Connection setup</h2>
      <p class="setup-body" data-i18n="setupBody">Pick what this VPN is for, the server it rides, and how it carries your traffic.</p>''',
'''      <h2 data-i18n="setupTitle">Connection setup</h2>''')

# Settings: compact rows, chip controls, kill switch and background in one
# card, about pinned to the bottom.
i = t.index('  <section class="view" id="view-settings">')
j = t.index('</main>')
NEW = '''  <section class="view" id="view-settings">
    <div class="set-head">
      <h1 data-i18n="tabSettings">Settings</h1>
      <p data-i18n="settingsSub">Customize your QuotaVPN experience.</p>
    </div>

    <button class="sec setting" id="row-routing">
      <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h9"/><path d="M17 6h3"/><path d="M4 12h3"/><path d="M11 12h9"/><path d="M4 18h7"/><path d="M15 18h5"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="13" cy="18" r="2"/></svg></span>
      <span class="set-txt"><b data-i18n="routing">App routing</b><i id="routing-sub"></i></span>
      <svg class="chev" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6"/></svg>
    </button>

    <div class="sec setting">
      <div class="set-row">
        <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.5 2.6 3.8 5.6 3.8 9S14.5 18.4 12 21c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z"/></svg></span>
        <span class="set-txt"><b data-i18n="language">Language</b><i data-i18n="languageBody">Choose your preferred language.</i></span>
        <div class="seg langseg" id="lang-seg"><button data-lang="en" class="on">EN</button><button data-lang="ar">عربي</button></div>
      </div>
    </div>

    <div class="sec setting">
      <div class="set-row">
        <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l7 3v6c0 4.4-3 7.6-7 9-4-1.4-7-4.6-7-9V6z"/><path d="M9.5 12l1.8 1.8L15 10"/></svg></span>
        <span class="set-txt"><b data-i18n="ksTitle">Kill switch</b><i data-i18n="ksHint">Kill switch: turn on Always-on VPN in the system settings. If the VPN drops, internet stops instead of leaking.</i></span>
        <button id="btn-vpn-settings" class="chip" data-i18n="openVpnSettings">Open</button>
      </div>
      <div class="set-row">
        <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="7" y="2.5" width="10" height="19" rx="2.5"/><path d="M11 5.5h2"/><path d="M12 18.5h.01"/></svg></span>
        <span class="set-txt"><b data-i18n="bgTitle">Background running</b><i id="bg-hint" data-i18n="bgHint">Keep the VPN on when you swipe the app away.</i></span>
        <button id="btn-bg" class="chip" data-i18n="bgBtnAllow">Allow</button>
      </div>
    </div>

    <div class="sec setting">
      <div class="set-row">
        <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.63-6.36"/><path d="M21 3v6h-6"/></svg></span>
        <span class="set-txt"><b data-i18n="updTitle">Updates</b><i id="upd-state" data-i18n="updIdle">Not checked yet.</i></span>
        <button id="btn-check-upd" class="chip" data-i18n="updCheck">Check</button>
        <button id="btn-get-upd" class="chip brand" data-i18n="updGet" hidden>Install</button>
      </div>
    </div>

    <div class="sec setting about">
      <div class="set-row">
        <span class="set-ico" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 7.5h.01"/></svg></span>
        <span class="set-txt"><b data-i18n="aboutTitle">About</b><i data-i18n="aboutBody">The build you are running and the server it rides.</i></span>
      </div>
      <div class="srvrow"><span data-i18n="versionLbl">Version</span><b id="set-version" dir="ltr">-</b></div>
      <div class="srvrow"><span data-i18n="cardForVpn">Server</span><b id="set-server" dir="ltr">-</b></div>
    </div>
  </section>

'''
t = t[:i] + NEW + t[j:]
io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("html ok")

# ----------------------------------------------------------------- css -----
P = UI + r"\style.css"
t = io.open(P, encoding="utf-8", newline="").read().replace("\r\n", "\n")

old_start = t.index("/* Settings: a header, then one card per setting")
old_end = t.index("/* Speed history:", old_start)
NEWCSS = '''/* Settings: a header, then one card per setting - icon chip, title, one
   quiet line, the control as a chip at the end of the row. Rows are 52 tall
   on a hairline rhythm; the About card closes the page at its bottom. */
.set-head { margin: 0 2px 10px; }
.set-head h1 { margin: 0; font-size: 19px; line-height: 1.25; font-weight: 600; letter-spacing: -0.01em; color: var(--txt); }
.set-head p { margin: 3px 0 0; font-size: 12.5px; line-height: 1.4; color: var(--txt2); }
.setting { padding: 8px 14px; }
.set-row { display: flex; align-items: center; gap: 10px; min-width: 0; min-height: 58px; padding: 8px 0; }
.set-row + .set-row { border-top: 1px solid var(--line); }
.set-ico { width: 32px; height: 32px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 10px; background: rgb(255 255 255 / 0.03); color: var(--accent-strong); }
.set-ico svg { width: 15px; height: 15px; }
.set-txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.set-txt b { font-size: 13.5px; font-weight: 600; line-height: 1.3; color: var(--txt); }
.set-txt i { font-style: normal; font-size: 11.5px; line-height: 1.35; color: var(--txt3); }
.setting .chev { width: 16px; height: 16px; color: var(--txt3); flex: none; }
button.setting { width: 100%; text-align: start; }
.chip { flex: none; min-height: 34px; padding: 0 12px; border-radius: 10px; border: 1px solid var(--line-strong); background: rgb(255 255 255 / 0.02); color: var(--txt); font-size: 12.5px; font-weight: 600; }
.chip.brand { border-color: var(--accent-line); background: var(--accent-bg); color: var(--accent-strong); }
.chip[hidden] { display: none; }
.setting.about { margin-top: auto; }
.setting.about .srvrow { border-top: 1px solid var(--line); padding: 9px 2px; margin: 0; }
.setting.about .srvrow:last-child { padding-bottom: 10px; }
.setting.about .srvrow b { text-align: end; }

'''
t = t[:old_start] + NEWCSS + t[old_end:]

def sub(old, new, cnt=1):
    global t
    assert t.count(old) == cnt, "COUNT %d != %d FOR %s" % (t.count(old), cnt, old[:80])
    t = t.replace(old, new, cnt)

# Speed fills its screen: the card takes the free height and its blocks
# spread across it, the history row closes the page.
sub(".speed-bars{display:flex; align-items:flex-end; gap:2px; height:56px;",
    ".speed-bars{display:flex; align-items:flex-end; gap:2px; height:84px;")
sub(".speed-sec .iconbtn{",
    ".speed-sec{flex:1 1 auto; display:flex; flex-direction:column; justify-content:space-between}\n.speed-sec .iconbtn{")

# Home: the transport group sinks to the card's bottom so the card reads as
# one composed panel instead of a stack, in both states.
sub("#transport-seg { flex: none; align-self: stretch; min-width: 0; padding: 3px; border-radius: var(--r-ctl); }",
    "#transport-label { margin-top: auto; }\n#transport-seg { flex: none; align-self: stretch; min-width: 0; padding: 3px; border-radius: var(--r-ctl); }")

io.open(P, "w", encoding="utf-8", newline="").write(t.replace("\n", "\r\n"))
print("css ok")
