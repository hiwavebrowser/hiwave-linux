#!/usr/bin/env python3
"""Z2-T1 measurement. TEST ONLY (Pete's amendment, relayed by Atlas #632/#633).
Variant H: curl_cffi Chrome TLS ClientHello + HTTP/2 settings (the handshake
shape only), with the HONEST HiWave UA and the same browser-shaped header set the
engine would send (no sec-ch-ua, no Chrome brand). One load per site per run, GET /,
no challenge answering (a challenge page is recorded as BLOCK)."""
import sys, json, time
from curl_cffi import requests
sys.path.insert(0, '.')
from waf_probe import SITES, classify, HIWAVE_UA
TARGET = sys.argv[2] if len(sys.argv) > 2 else "chrome"
HDRS = {
 "User-Agent": HIWAVE_UA,
 "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
 "Accept-Language": "en-US,en;q=0.9",
 "Accept-Encoding": "gzip, deflate",
 "Upgrade-Insecure-Requests": "1",
 "Sec-Fetch-Dest": "document", "Sec-Fetch-Mode": "navigate",
 "Sec-Fetch-Site": "none", "Sec-Fetch-User": "?1",
}
out = {}
for site, (host, vendor) in SITES.items():
    try:
        r = requests.get(f"https://{host}/", headers=HDRS, impersonate=TARGET,
                         default_headers=False, timeout=20, allow_redirects=True)
        body = r.text
        verdict, detail = classify(r.status_code, body, dict(r.headers), vendor)
        out[site] = {"vendor": vendor, "verdict": verdict, "detail": detail, "status": r.status_code,
                     "http_version": getattr(r, "http_version", None), "bytes": len(r.content), "final": r.url}
    except Exception as e:
        out[site] = {"vendor": vendor, "verdict": "ERR", "detail": type(e).__name__ + ": " + str(e)[:80]}
    o = out[site]
    print(f"{site:12} {vendor:10} {o['verdict']:6} {o['detail']:28} h={o.get('http_version')} bytes={o.get('bytes')}", flush=True)
    time.sleep(0.5)
json.dump({"target": TARGET, "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "results": out}, open(sys.argv[1], "w"), indent=1)
print("PASS", sum(1 for v in out.values() if v["verdict"] == "PASS"), "/", len(out))
