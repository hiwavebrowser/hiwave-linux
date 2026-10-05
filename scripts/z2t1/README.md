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
