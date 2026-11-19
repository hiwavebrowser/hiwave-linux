#!/usr/bin/env python3
"""Set (b) of the three-way comparison, Linux column: websuite/cases (8) + websuite/micro (21).
Each page: RustKit (parity-capture --html-file) and Chrome for Testing 148 (chrome_board.mjs, no
actions), both under scripts/board-fonts.conf, at the campaign registry's size or 1280x800."""
import glob, json, os, subprocess, sys
REPO = "/home/petec/repos/hiwave-linux-3way"
CAP = "/home/petec/repos/.talos-target/3way/release/parity-capture"
W = "/home/petec/repos/.talos-wt/threeway/websuite"
G = "/home/petec/repos/gate-runner"
os.chdir(REPO)
env = dict(os.environ)
for k in ("DISPLAY", "WAYLAND_DISPLAY"):
    env.pop(k, None)
env.update(PATH=f"{G}/tools/node/bin:" + env["PATH"], PLAYWRIGHT_BROWSERS_PATH=os.path.expanduser("~/.cache/ms-playwright"),
           PARITY_CHROME_PATH=f"{G}/tools/cft/chrome-linux64/chrome", XDG_DATA_HOME=f"{W}/d", XDG_CONFIG_HOME=f"{W}/c",
           XDG_CACHE_HOME=f"{W}/k", FONTCONFIG_FILE=f"{REPO}/scripts/board-fonts.conf")
reg = {c["html"]: (n, c["width"], c["height"], c.get("capture_args", [])) for n, c in json.load(open("cases/registry.json"))["cases"].items()}
pages = sorted(glob.glob("websuite/cases/*/index.html")) + sorted(glob.glob("websuite/micro/*/*.html"))
pairs, meta = [], []
for p in pages:
    name = p.replace("websuite/", "").replace("/index.html", "").replace(".html", "").replace("/", "__")
    _, w, h, extra = reg.get(p, (name, 1280, 800, []))
    d = f"{W}/pages/{name}"; os.makedirs(d, exist_ok=True)
    rk = subprocess.run([CAP, "--html-file", p, "--width", str(w), "--height", str(h), "--dump-frame", f"{d}/rustkit.png", *extra],
                        capture_output=True, text=True, timeout=180, env=env)
    ch = subprocess.run(["node", "tools/parity_oracle/chrome_board.mjs", f"{REPO}/{p}", f"{d}/chrome", str(w), str(h)],
                        capture_output=True, text=True, timeout=180, env=env)
    cj = {}
    try: cj = json.load(open(f"{d}/chrome/chrome.json"))
    except Exception: pass
    meta.append({"case": name, "html": p, "width": w, "height": h, "size_from": "registry" if p in reg else "default 1280x800",
                 "rustkit_exit": rk.returncode, "chrome_exit": ch.returncode, "chrome_version": cj.get("browser"),
                 "chrome_page_errors": cj.get("page_errors", [])})
    pairs.append({"case": name, "rustkit": f"{d}/rustkit.png", "chrome": f"{d}/chrome/chrome.png"})
    print(name, w, h, rk.returncode, ch.returncode, flush=True)
json.dump(pairs, open(f"{W}/pairs.json", "w")); json.dump(meta, open(f"{W}/meta.json", "w"), indent=2)
