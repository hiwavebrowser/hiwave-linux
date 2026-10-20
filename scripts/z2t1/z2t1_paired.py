#!/usr/bin/env python3
"""Z2-T1 paired measurement, TEST ONLY. One load per site per variant, GET /, no challenge answering.
 C = control: OpenSSL ClientHello, ALPN http/1.1, HTTP/1.1, honest HiWave UA + browser-shaped headers.
 H = handshake: curl_cffi Chrome ClientHello + HTTP/2 settings, SAME honest UA + SAME headers (no sec-ch-ua)."""
import sys, json, re, time
from curl_cffi import requests
sys.path.insert(0, '.')
import waf_probe as W
HDRS = {"User-Agent": W.HIWAVE_UA,
 "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
 "Accept-Language": "en-US,en;q=0.9", "Accept-Encoding": "gzip, deflate", "Upgrade-Insecure-Requests": "1",
 "Sec-Fetch-Dest": "document", "Sec-Fetch-Mode": "navigate", "Sec-Fetch-Site": "none", "Sec-Fetch-User": "?1"}
BAD = re.compile(r"just a moment|access denied|pardon our interruption|robot or human|captcha|attention required|are you a human|verify you are|denied|blocked|unusual traffic|security check", re.I)
def verdict(status, body, headers):
    t = re.search(r"<title[^>]*>(.*?)</title>", body[:200000], re.I | re.S)
    title = re.sub(r"\s+", " ", t.group(1)).strip()[:80] if t else ""
    h = str(headers).lower()
    chal = ("cf-mitigated" in h) or "challenge-platform" in body[:6000].lower() or "captcha-delivery.com" in body[:6000].lower() or "/_sec/cp_challenge" in body[:6000].lower()
    ok = status == 200 and not chal and not BAD.search(title) and len(body) > 5000
    return ("PASS" if ok else "BLOCK"), title, chal
def ctrl(host):
    st, body, hd, note = W.fetch(host, W.hiwave_request(host), ["http/1.1"])
    if note: return {"verdict": "ERR", "detail": note}
    v, title, chal = verdict(st, body, hd); return {"verdict": v, "status": st, "bytes": len(body), "title": title, "challenge": chal}
def hand(host, target):
    r = requests.get(f"https://{host}/", headers=HDRS, impersonate=target, default_headers=False, timeout=25, allow_redirects=True)
    v, title, chal = verdict(r.status_code, r.text, r.headers)
    return {"verdict": v, "status": r.status_code, "bytes": len(r.content), "title": title, "challenge": chal, "http_version_code": r.http_version, "final": r.url}
target = sys.argv[2] if len(sys.argv) > 2 else "chrome"
out = {}
for site, (host, vendor) in W.SITES.items():
    row = {"vendor": vendor}
    for name, fn in (("C", lambda: ctrl(host)), ("H", lambda: hand(host, target))):
        try: row[name] = fn()
        except Exception as e: row[name] = {"verdict": "ERR", "detail": f"{type(e).__name__}: {str(e)[:70]}"}
        time.sleep(0.5)
    out[site] = row
    c, h = row["C"], row["H"]
    print(f"{site:12} {vendor:10} C={c['verdict']}/{c.get('status','-')}  H={h['verdict']}/{h.get('status','-')} {h.get('bytes','-')}B  '{h.get('title','')[:40]}'", flush=True)
json.dump({"target": target, "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "results": out}, open(sys.argv[1], "w"), indent=1)
for k in "CH": print(k, "PASS", sum(1 for v in out.values() if v[k]["verdict"] == "PASS"), "/", len(out))
