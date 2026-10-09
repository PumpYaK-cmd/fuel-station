import pathlib

HERE = pathlib.Path(__file__).parent

PROJECTS = ["comi-revival", "forge-load", "fuel-station", "hwx-clankers", "kitsu-pen", "shototsu", "yak-automotive", "yak-aviation"]
COLS = [
    ("BACKLOG", 44, "", [
        ("YA2-151", "Cockpit aim follow: pick the yaw-limit side from the aim, reset free look correctly", None, None),
        ("YA2-150", "Blend the elevator at touchdown from the rate law to the stick", None, None),
        ("YA2-146", "Remove dead LEX level from Vapour.levels", None, None),
        ("YA2-144", "Control surface clips: visual rate limit lags the physics", None, None),
        ("YA2-145", "STOVL: thrust step when leaving hover mode 2", None, None),
        ("YA2-142", "STOVL lift fan vane fore/aft vectoring", None, None),
        ("YA2-141", "STOVL hover jitter", None, None),
        ("YA2-140", "Animate SwitchHookStovl cockpit piece", None, None),
    ]),
    ("READY", 15, "", [
        ("YA2-16", "4.4 Per-element stall and flat-plate post-stall", "T2", None),
        ("YA2-18", "4.5 Mach effects", "T1", None),
        ("YA2-23", "4.10 Aero tests: C172 known answer, F-35 trim and stall speed", "T2", None),
        ("YA2-19", "4.7 Ground effect", "T1", None),
        ("YA2-24", "5.3 Fuel tanks, consumption, CG shift", "T1", None),
        ("YA2-25", "5.2 Afterburner stages: thrust and fuel flow", "T1", None),
    ]),
    ("DOING", 2, "doing", [
        ("YA2-47", "Dump tool drops duplicate-named cfg sections ([FLAPS.0])", "T1", ("fuel", "\uf0298", "fuel-3")),
        ("YA2-44", "dump_params.py: title from package folder, not 'source'", "T1", ("fuel", "\uf0298", "fuel-1")),
    ]),
    ("REVIEW", 1, "review", [
        ("YA2-132", "HMD: hide the flight HUD while the PCD zoom is on", "T2", ("mech", "\uf0ad", "mechanic")),
    ]),
    ("DONE", 92, "done", [
        ("YA2-42", "Previous engine (F-22 per-surface aero) removed", None, None),
        ("YA2-43", "11.1 + 11.2 Onboard and toolchain", None, None),
        ("YA2-1", "[HUMAN] Decide D1-D5: MSFS model, integrator, units, step, first-build scope", None, "human"),
        ("YA2-2", "1.1 Get the F-35B flight_model.cfg and engines.cfg, dump params", None, None),
        ("YA2-7", "3.1 Units module", None, None),
        ("YA2-3", "1.2 F35B config module from the cfg (MSFS keys and units)", None, None),
    ]),
]
ACTIVITY = [
    ("21:19", "\uf0298", "YA2-47", "gate green, submitting", "ok"),
    ("21:18", "\uf0ad", "YA2-132", "Review: 2 findings", "bad"),
    ("21:17", "\uf0298", "YA2-44", "plan written, implementing", ""),
    ("21:15", "\uf007", "", "Updated project YA2", "muted"),
    ("21:14", "\uf007", "", "imported 151 cards from trello", "muted"),
]

def card(c, i):
    key, title, tier, holder = c
    cls = "card"
    if holder == "human":
        cls += " human"; holder = None
    if i == 0 and key == "YA2-47":
        cls += " selected"
    top = f'<div class="top"><span class="key">{key}</span>' + (f'<span class="tier">[{tier}]</span>' if tier else "") + "</div>"
    hold = f'<div class="holder"><span class="glyph">{holder[1]}</span> {holder[2]}</div>' if holder else ""
    return f'<div class="{cls}" style="--i:{i}">{top}<div class="title">{title}</div>{hold}</div>'

def body(variant_extra=""):
    sidebar = "".join(f'<button class="proj{" sel" if p == "yak-aviation" else ""}">{p}</button>' for p in PROJECTS)
    cols = ""
    for name, n, cls, cards in COLS:
        cols += f'<section class="col {cls}"><header><span class="name">{name}</span><span class="count">{n}</span><span class="gauge"><i style="--fill:{min(n,20)/20}"></i></span></header><div class="list">{"".join(card(c, i) for i, c in enumerate(cards))}</div></section>'
    act = "".join(f'<div class="ev {cls}"><span class="t">{t}</span><span class="glyph">{g}</span><span class="k">{k}</span><span class="m">{m}</span></div>' for t, g, k, m, cls in ACTIVITY)
    return f'''
<div class="bg">{variant_extra}</div>
<div class="titlebar">
  <div class="brand"><span class="logo">\uf0298</span> FUEL STATION</div>
  <div class="spacer"></div>
  <div class="agents"><span class="pulse"></span> 3 agents <span class="agent">\uf0298 fuel-3 YA2-47</span><span class="agent">\uf0298 fuel-1 YA2-44</span><span class="agent">\uf0ad mechanic YA2-132</span></div>
  <div class="win"><span>\uf2d1</span><span>\uf2d0</span><span class="close">\uf00d</span></div>
</div>
<div class="shell">
  <aside class="sidebar">
    <div class="projects">{sidebar}</div>
    <button class="proj add">+ Add Project</button>
  </aside>
  <main>
    <div class="board-head"><h1>YA2 <span class="dot">·</span> yak-aviation</h1><div class="spacer"></div><button class="btn">Repair setup</button></div>
    <div class="board">{cols}</div>
    <section class="activity"><header><span class="name">ACTIVITY</span><span class="live">LIVE</span></header><div class="feed">{act}</div></section>
  </main>
</div>
<footer><span>C:\\Users\\gabri\\AppData\\Roaming\\FuelStation\\station.db <span class="dot">·</span> data_version 2</span><div class="spacer"></div><span class="btn small">Station Night \u25be</span></footer>
'''

BASE_CSS = '''
*{box-sizing:border-box;margin:0;padding:0}
:root{--bg:#0A0E14;--surface:#0F1520;--hover:#151D2B;--border:#1C2636;--border2:#2A3A52;--text:#C8D3E6;--muted:#5E6E88;--primary:#3D8BFF;--accent:#6CC4FF;--ok:#2FD3A0;--warn:#F0B44C;--bad:#FF5C6C}
html,body{height:100%;background:var(--bg);color:var(--text);font:13px/1.35 "JetBrainsMono Nerd Font","JetBrainsMonoNerdFont-Regular","JetBrains Mono",ui-monospace,monospace;overflow:hidden}
body{display:flex;flex-direction:column;position:relative}
.bg{position:fixed;inset:0;z-index:0;overflow:hidden;pointer-events:none}
.titlebar,.shell,footer{position:relative;z-index:1}
.titlebar{height:36px;display:flex;align-items:center;gap:24px;padding:0 0 0 16px;font-size:12px;letter-spacing:.08em;user-select:none}
.brand{color:var(--text);display:flex;gap:8px;align-items:center}
.brand .logo{color:var(--accent)}
.crumb{color:var(--muted)}
.spacer{flex:1}
.dot{color:var(--muted)}
.agents{color:var(--muted);display:flex;gap:14px;align-items:center}
.agents .agent{color:var(--accent);letter-spacing:0}
.pulse{width:7px;height:7px;border-radius:50%;background:var(--accent);box-shadow:0 0 0 0 rgba(108,196,255,.6);animation:pulse 2s infinite}
@keyframes pulse{0%{box-shadow:0 0 0 0 rgba(108,196,255,.6)}70%{box-shadow:0 0 0 8px rgba(108,196,255,0)}100%{box-shadow:0 0 0 0 rgba(108,196,255,0)}}
.win{display:flex;height:100%}
.win span{width:46px;display:grid;place-items:center;color:var(--muted);transition:background .15s,color .15s}
.win span:hover{background:var(--hover);color:var(--text)}
.win .close:hover{background:var(--bad);color:#fff}
.shell{flex:1;display:flex;min-height:0}
.sidebar{width:200px;padding:20px 16px;display:flex;flex-direction:column;gap:20px;border-right:1px solid var(--border)}
.projects{display:flex;flex-direction:column;gap:6px;flex:1}
.proj,.btn{background:none;border:1px solid var(--primary);color:var(--primary);border-radius:2px;padding:4px 8px;font:inherit;text-align:center;cursor:pointer;transition:background .15s,transform .15s,box-shadow .3s}
.proj:hover,.btn:hover{background:var(--hover)}
.proj.sel{border-color:var(--accent);color:var(--accent)}
.btn.small{padding:2px 8px}
main{flex:1;display:flex;flex-direction:column;gap:14px;padding:16px 20px 12px;min-width:0}
.board-head{display:flex;align-items:center;gap:20px}
h1{font-size:13px;font-weight:normal}
.board{flex:1;display:grid;grid-template-columns:repeat(5,1fr);gap:10px;min-height:0}
.col{background:var(--surface);border:1px solid var(--border);border-radius:2px;padding:10px;display:flex;flex-direction:column;gap:10px;min-height:0;position:relative}
.col header{display:flex;gap:8px;align-items:baseline}
.col .count{color:var(--muted)}
.gauge{display:none}
.col.review .name{color:var(--warn)} .col.done .name{color:var(--ok)}
.list{overflow:hidden;display:flex;flex-direction:column;gap:6px}
.card{background:var(--surface);border:1px solid var(--border);border-radius:2px;padding:8px;cursor:pointer;transition:border-color .15s,background .15s,transform .15s;animation:rise .4s both;animation-delay:calc(var(--i)*40ms)}
@keyframes rise{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:none}}
.card:hover{background:var(--hover);border-color:var(--border2)}
.card .top{display:flex;justify-content:space-between;color:var(--muted);margin-bottom:4px}
.card.human{border-color:var(--warn)} .card.selected{border-color:var(--accent)}
.holder{color:var(--accent);margin-top:4px}
.activity{height:150px;background:var(--surface);border:1px solid var(--border);border-radius:2px;padding:10px;display:flex;flex-direction:column;gap:8px}
.activity header{display:flex;gap:10px;align-items:baseline}
.live{color:var(--ok);font-size:10px;letter-spacing:.1em;animation:blink 1.6s steps(1) infinite}
@keyframes blink{50%{opacity:.25}}
.ev{display:flex;gap:10px;animation:rise .4s both}
.ev .t{color:var(--muted)} .ev .glyph{color:var(--accent)} .ev .k{width:70px;color:var(--text)}
.ev.ok .m{color:var(--ok)} .ev.bad .m{color:var(--bad)} .ev.muted .m{color:var(--muted)}
footer{display:flex;align-items:center;gap:20px;padding:8px 20px;border-top:1px solid var(--border);color:var(--muted);font-size:12px}
'''

VARIANTS = {}

VARIANTS["1-orbit"] = ("Orbit", '''
<div class="stars s1"></div><div class="stars s2"></div><div class="stars s3"></div>
''', '''
.bg{background:radial-gradient(ellipse at 70% 110%, #16284A 0%, var(--bg) 60%)}
.stars{position:absolute;inset:-200px;background-repeat:repeat}
.s1{background-image:radial-gradient(1px 1px at 20px 30px,#fffc 0,transparent 1px),radial-gradient(1px 1px at 90px 120px,#fff9 0,transparent 1px),radial-gradient(1px 1px at 160px 60px,#fffa 0,transparent 1px),radial-gradient(1px 1px at 240px 200px,#fff8 0,transparent 1px),radial-gradient(1px 1px at 60px 210px,#fff7 0,transparent 1px),radial-gradient(1px 1px at 270px 40px,#fff6 0,transparent 1px);background-size:300px 260px;animation:drift 240s linear infinite}
.s2{background-image:radial-gradient(1.5px 1.5px at 50px 80px,#9cf9 0,transparent 1.5px),radial-gradient(1px 1px at 200px 20px,#fff7 0,transparent 1px),radial-gradient(1.5px 1.5px at 320px 150px,#fffa 0,transparent 1.5px);background-size:420px 380px;animation:drift 150s linear infinite}
.s3{background-image:radial-gradient(2px 2px at 100px 100px,#fff 0,transparent 2px),radial-gradient(2.5px 2.5px at 400px 300px,#bdf 0,transparent 2.5px),radial-gradient(2px 2px at 560px 80px,#fff 0,transparent 2px);background-size:700px 600px;animation:drift 90s linear infinite,twinkle 4s ease-in-out infinite alternate}
@keyframes drift{to{transform:translate(-300px,-120px)}}
@keyframes twinkle{from{opacity:.5}to{opacity:1}}
.titlebar{background:linear-gradient(to bottom,#0F1520EE,#0F152000);border-bottom:1px solid #1C263680}
.sidebar{background:#0F1520AA;border-right-color:#1C2636AA}
.col,.activity{background:#0F1520B8}
.card{background:#0F1520F0}
.card.selected{box-shadow:0 0 18px -6px var(--accent)}
.col.doing{border-color:#3D8BFF66;box-shadow:inset 0 0 30px -20px var(--primary)}
.col.doing .name{color:var(--primary)}
.col.doing::after{content:"";position:absolute;left:0;right:0;top:0;height:1px;background:linear-gradient(90deg,transparent,var(--accent),transparent);animation:scan 3s ease-in-out infinite}
@keyframes scan{0%{transform:translateX(-100%)}100%{transform:translateX(100%)}}
.holder .glyph{display:inline-block;animation:bob 1.6s ease-in-out infinite}
@keyframes bob{50%{transform:translateY(-2px)}}
footer{background:#0F1520AA}
''')

VARIANTS["2-dock"] = ("Docking Bay", '''
<div class="grid"></div><div class="sweep"></div>
''', '''
.bg{background:var(--bg)}
.grid{position:absolute;inset:0;background-image:linear-gradient(#2A3A5280 1px,transparent 1px),linear-gradient(90deg,#2A3A5280 1px,transparent 1px);background-size:40px 40px;-webkit-mask-image:radial-gradient(ellipse at 50% 40%,#000 30%,transparent 80%);mask-image:radial-gradient(ellipse at 50% 40%,#000 30%,transparent 80%)}
.sweep{position:absolute;left:0;right:0;top:0;background:linear-gradient(180deg,transparent 0,#3D8BFF14 50%,transparent 100%);height:200px;animation:sweep 9s linear infinite}
@keyframes sweep{from{transform:translateY(-200px)}to{transform:translateY(110vh)}}
.titlebar{border-bottom:1px solid var(--border);background:var(--surface);font-size:11px}
.titlebar .crumb::before{content:"BAY ";color:var(--primary)}
.col{background:#0F1520EE;border-color:transparent}
.col::before,.col::after,.col .list::before,.col .list::after{content:"";position:absolute;width:12px;height:12px;border:1px solid var(--border2);pointer-events:none}
.col::before{top:-1px;left:-1px;border-right:0;border-bottom:0}
.col::after{top:-1px;right:-1px;border-left:0;border-bottom:0}
.col .list::before{bottom:-1px;left:-1px;border-right:0;border-top:0}
.col .list::after{bottom:-1px;right:-1px;border-left:0;border-top:0}
.col.doing::before,.col.doing::after,.col.doing .list::before,.col.doing .list::after{border-color:var(--primary);animation:brk 2s ease-in-out infinite}
@keyframes brk{50%{opacity:.4}}
.col header{border-bottom:1px solid var(--border);padding-bottom:8px;letter-spacing:.1em;font-size:11px}
.gauge{display:block;flex:1;height:3px;background:var(--border);align-self:center;position:relative;overflow:hidden}
.gauge i{position:absolute;inset:0;transform-origin:left;transform:scaleX(var(--fill));background:var(--primary);animation:fill 1s cubic-bezier(.2,.8,.2,1) both}
.col.review .gauge i{background:var(--warn)} .col.done .gauge i{background:var(--ok)}
@keyframes fill{from{transform:scaleX(0)}}
.card{border-left:2px solid var(--border2);background:#0A0E14}
.card:hover{border-left-color:var(--primary)}
.card.selected{border-left-color:var(--accent);border-color:var(--accent)}
.card.human{border-left-color:var(--warn)}
.holder{font-size:11px;letter-spacing:.05em}
.activity{background:#0F1520EE}
.ev .t::after{content:" \\2502";color:var(--border2)}
footer{background:var(--surface);letter-spacing:.05em;font-size:11px}
''')

VARIANTS["3-nebula"] = ("Nebula", '''
<div class="neb a"></div><div class="neb b"></div><div class="neb c"></div><div class="stars"></div><div class="planet"></div>
''', '''
.bg{background:#070A10}
.neb{position:absolute;width:70vw;height:70vw;border-radius:50%;opacity:.9;mix-blend-mode:screen}
.a{background:radial-gradient(circle,#2746A8 0,transparent 68%);left:-15vw;top:-20vw;animation:float 40s ease-in-out infinite alternate}
.b{background:radial-gradient(circle,#5B2AA8 0,transparent 68%);right:-20vw;bottom:-25vw;animation:float 55s ease-in-out infinite alternate-reverse}
.c{background:radial-gradient(circle,#0E8A9E 0,transparent 68%);left:35vw;top:30vh;width:45vw;height:45vw;opacity:.5;animation:float 70s ease-in-out infinite alternate}
@keyframes float{to{transform:translate(8vw,6vh) scale(1.15)}}
.stars{position:absolute;inset:-100px;background-image:radial-gradient(1px 1px at 30px 40px,#fff 0,transparent 1px),radial-gradient(1px 1px at 130px 90px,#fffa 0,transparent 1px),radial-gradient(2px 2px at 220px 160px,#cdf 0,transparent 2px),radial-gradient(1px 1px at 300px 30px,#fff9 0,transparent 1px),radial-gradient(1px 1px at 80px 250px,#fff8 0,transparent 1px);background-size:360px 300px;animation:drift 200s linear infinite}
@keyframes drift{to{transform:translate(-360px,-150px)}}
.planet{position:absolute;right:-240px;bottom:-520px;width:900px;height:900px;border-radius:50%;background:radial-gradient(circle at 35% 30%,#2A4A8A,#0A0E14 62%);box-shadow:0 0 160px -10px #3D8BFF88,inset 60px 30px 160px -40px #6CC4FF55;animation:spin 240s linear infinite}
@keyframes spin{to{transform:rotate(360deg)}}
.titlebar{background:linear-gradient(90deg,#0F1520CC,#0F152066);border-bottom:1px solid #2A3A5266}
.brand .logo{text-shadow:0 0 12px var(--accent)}
.sidebar{background:#0F152088;border-right-color:#2A3A5255}
.proj.sel{box-shadow:0 0 14px -4px var(--accent),inset 0 0 14px -10px var(--accent)}
.col,.activity{background:linear-gradient(180deg,#0F1520D0,#0F152099);border-color:#2A3A5266}
.col header .name{text-shadow:0 0 10px currentColor}
.card{background:#0A0E14CC;border-color:#2A3A5280}
.card:hover{transform:translateY(-1px);border-color:var(--primary);box-shadow:0 6px 20px -10px var(--primary)}
.card.selected{box-shadow:0 0 24px -8px var(--accent);border-color:var(--accent)}
.col.doing{border-color:#3D8BFF88;box-shadow:0 0 40px -24px var(--primary)}
.holder{position:relative;padding-left:18px}
.holder .glyph{position:absolute;left:0;top:0;animation:orbit 3s linear infinite}
@keyframes orbit{0%{transform:translate(0,0)}25%{transform:translate(2px,-2px)}50%{transform:translate(0,-3px)}75%{transform:translate(-2px,-2px)}100%{transform:translate(0,0)}}
.live{text-shadow:0 0 8px var(--ok)}
footer{background:#0F152088;border-top-color:#2A3A5255}
''')

VARIANTS["4-control"] = ("Mission Control", '''
<div class="horizon"></div><div class="stars"></div>
''', '''
.bg{background:linear-gradient(180deg,#0A0E14 0%,#0B111B 70%,#0E1A2B 100%)}
.stars{position:absolute;inset:0;background-image:radial-gradient(1px 1px at 40px 50px,#fffb 0,transparent 1px),radial-gradient(1px 1px at 180px 20px,#fff5 0,transparent 1px),radial-gradient(1px 1px at 260px 130px,#fff6 0,transparent 1px);background-size:320px 220px;opacity:.6}
.horizon{position:absolute;left:-10%;right:-10%;bottom:-62vh;height:80vh;border-radius:50%;border-top:1px solid #3D8BFFAA;box-shadow:0 -60px 160px -30px #3D8BFF77;background:radial-gradient(ellipse at 50% 0,#17305A 0,#0A0E14 60%)}
.titlebar{background:var(--surface);border-bottom:1px solid var(--border);font-size:11px;letter-spacing:.14em;gap:0;padding:0}
.titlebar>*{padding:0 18px;height:100%;display:flex;align-items:center;border-right:1px solid var(--border)}
.titlebar .spacer{border:0}
.titlebar .win{padding:0;border:0}
.brand{background:#3D8BFF14}
.agents::before{content:"TELEMETRY";color:var(--muted);margin-right:12px;letter-spacing:.14em}
.sidebar{background:#0F1520CC}
.proj{text-align:left;border-color:var(--border2);color:var(--text)}
.proj::before{content:"\\25b8 ";color:var(--muted)}
.proj.sel{border-color:var(--accent);color:var(--accent)}
.proj.sel::before{color:var(--accent)}
.proj.add{text-align:center;border-color:var(--primary);color:var(--primary)}
.proj.add::before{content:""}
.col{background:#0F1520E6;border-top:2px solid var(--border2)}
.col.doing{border-top-color:var(--primary)} .col.review{border-top-color:var(--warn)} .col.done{border-top-color:var(--ok)}
.col header{letter-spacing:.12em;font-size:11px;align-items:center}
.col header::before{content:"";width:6px;height:6px;border-radius:50%;background:var(--border2)}
.col.doing header::before{background:var(--primary);box-shadow:0 0 8px var(--primary);animation:led 1.2s steps(2) infinite}
.col.review header::before{background:var(--warn)} .col.done header::before{background:var(--ok)}
@keyframes led{50%{opacity:.3}}
.card{background:#0A0E14;border-color:var(--border)}
.card .key{color:var(--primary)}
.card:hover{border-color:var(--primary);background:#0F1520}
.card.selected{border-color:var(--accent);background:#3D8BFF10}
.card.selected .title::before{content:"> ";color:var(--accent)}
.holder{font-size:11px;letter-spacing:.06em}
.holder .glyph{animation:led 1s steps(2) infinite}
.activity{background:#0A0E14F0;font-size:12px}
.activity header .name::after{content:" \\2588";animation:blink 1s steps(1) infinite;color:var(--ok);font-size:10px}
.ev .t{color:var(--primary)}
footer{background:var(--surface);font-size:11px;letter-spacing:.08em}
''')

INDEX = '''<!doctype html><meta charset="utf-8"><title>Fuel Station mockups</title>
<style>body{margin:0;background:#0A0E14;color:#C8D3E6;font:14px ui-monospace,monospace;display:grid;grid-template-columns:1fr 1fr;grid-auto-rows:50vh}
a{display:block;position:relative;border:1px solid #1C2636;overflow:hidden;color:inherit;text-decoration:none}
iframe{width:200%;height:200%;transform:scale(.5);transform-origin:0 0;border:0;pointer-events:none}
span{position:absolute;left:12px;top:10px;background:#0F1520;border:1px solid #3D8BFF;padding:3px 8px;color:#6CC4FF}
a:hover{border-color:#6CC4FF}</style>
%s'''

for slug, (name, extra, css) in VARIANTS.items():
    html = f'<!doctype html><html><head><meta charset="utf-8"><title>Fuel Station - {name}</title><style>{BASE_CSS}{css}</style></head><body>{body(extra)}</body></html>'
    (HERE / f"{slug}.html").write_text(html, encoding="utf-8")
links = "".join(f'<a href="{slug}.html" target="_blank"><iframe src="{slug}.html"></iframe><span>{slug[0]} - {name}</span></a>' for slug, (name, _, _) in VARIANTS.items())
(HERE / "index.html").write_text(INDEX.replace("%s", links), encoding="utf-8")
print("ok")
