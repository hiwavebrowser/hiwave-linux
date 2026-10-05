# Z2-T1 measurement (test-only handshake profile)

Basis: Pete's amendment, "You are free to fake the fingerprint. Its for the greater good. Only for testing though".
Scope: handshake SHAPE only (TLS ClientHello + h2 SETTINGS). Honest User-Agent unchanged, no challenge answered,
no script run, one `GET /` per site per run, n=1 per row (about +-1 site of noise: edmunds, yelp, doordash flip between runs).

Reproduce (needs libclang headers for bindgen):
`BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/15/include cargo run -p rustkit-http --features impersonate-test --example site_probe -- --chrome-handshake <hosts>`

| Client | PASS / 15 | Raw |
|---|---|---|
| Engine as shipped (rustls, h2) | 5 | raw/z2t1_engine_baseline.jsonl |
| Engine + Chrome handshake, run 1 (no ECH GREASE) | 7 | raw/z2t1_engine_chrome.jsonl |
| Engine + Chrome handshake, run 2 (final profile) | 6 | raw/z2t1_engine_chrome_final.jsonl |
| curl_cffi Chrome handshake, honest UA (reference) | 9 | raw/z2t1_paired.json (H) |
| OpenSSL/HTTP1.1 control (not the engine) | 4 | raw/z2t1_paired.json (C) |

ebay.com is 403 on every profile (Akamai Bot Manager; the handshake alone does not open it).
sap and nytimes pass for curl_cffi but not the engine profile. ClientHello diff vs curl_cffi Chrome
(raw/clienthello_*.json, captured on a local listener, no site traffic): identical except curl_cffi also sends
signature algorithms 0x0904-0x0906. Not tested as the cause. The h2 crate cannot set pseudo-header order
(Chrome: m,a,s,p; h2: m,s,a,p) or HEADERS priority; a throwaway patched h2 with Chrome pseudo order did not flip
sap or nytimes (4 extra exploratory loads, declared; not committed).

Extra loads beyond one per site per run, declared: first dry run, 3 ebay profile loads, the engine baseline run,
the two clienthello captures (local only), and the 4 patched-h2 loads above.

## Correction (same day, after Atlas asked for one more look at the h2 frames)
**Probe confound.** `site_probe` used a bare `Client::get`, which sends no `Accept-Language`; the shipped loader
(`rustkit-net`, default `en-US,en;q=0.9`) and the curl_cffi reference both send it. Every engine row above
(5 / 7 / 6) therefore lacked a header the real app sends. `site_probe` now sends it.

**h2 frames, local listener, no site traffic** (`h2_dump.py`, `raw/h2frames_*.json`): SETTINGS
(1=65536, 2=0, 4=6291456, 6=262144) and the connection WINDOW_UPDATE (+15663105) are byte-identical to curl_cffi
Chrome. Remaining HEADERS-frame differences: curl_cffi sets the PRIORITY flag (exclusive, dep 0, weight 256) and
pseudo order m,a,s,p (engine: m,s,a,p, h2 crate fixed); header names/order/values otherwise identical once
Accept-Language is sent. So SETTINGS/WINDOW_UPDATE are not the residual gap.

**Re-run with Accept-Language** (`raw/z2t1_engine_shipped_AL.jsonl`, `raw/z2t1_engine_chrome_AL.jsonl`): 2/15 shipped,
3/15 Chrome handshake. NOT comparable with the earlier rows: Cloudflare sites (cars, topps, indeed, glassdoor,
chrono24) now serve challenges to both clients, which says this egress IP's reputation had degraded after the
volume of loads in this campaign, so run-to-run drift exceeds the +-1 noise estimate. The one signal that survives
is **nytimes PASS on both clients once Accept-Language is sent**: the earlier "handshake opens nytimes" reading was
the missing header, not the handshake. No further live loads from this IP; a clean re-measure through the real
loader (rustkit-net) from a cooled egress is the right next step, not more probing here.
