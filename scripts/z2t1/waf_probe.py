#!/usr/bin/env python3
"""Network-lane diagnosis probe: which wire signal trips which WAF vendor.

Axes isolated WITHIN one TLS stack (python ssl/OpenSSL) so header effects are
attributable; the ALPN axis toggles only the ClientHello ALPN extension.
This does NOT reproduce native-tls's exact ClientHello — it measures the
HEADER and ALPN axes. The UA axis runs inside every variant pair.

Variants:
  A rustkit-verbatim : byte-faithful reproduction of rustkit-http's request
                       (h1, Connection: close, Accept: */*, mixed casing,
                        accept-language after Connection), ALPN absent.
  B rustkit+alpn     : same bytes, ALPN http/1.1 offered (extension present).
  C hiwave-headers   : HiWave UA + full browser-shaped header set in browser
                       order/casing, keep-alive, h1, ALPN http/1.1.
  D hiwave+h2alpn    : same as C but ALPN offers h2+http/1.1 (we still speak
                       h1 if h2 selected -> recorded as h2-selected, no HTTP
                       verdict; the axis measured is ClientHello acceptance).

HiWave UA is COHERENT AND HONEST: HiWave/1.0 with platform, never Chrome.
"""
import socket, ssl, sys, json, time

SITES = {
    # site: (host, vendor)
    "chatgpt":     ("chatgpt.com", "cloudflare"),
    "cars":        ("www.cars.com", "cloudflare"),
    "topps":       ("www.topps.com", "cloudflare"),
    "indeed":      ("www.indeed.com", "cloudflare"),
    "glassdoor":   ("www.glassdoor.com", "cloudflare"),
    "doordash":    ("www.doordash.com", "cloudflare"),
    "chrono24":    ("www.chrono24.com", "cloudflare"),
    "ebay":        ("www.ebay.com", "akamai"),
    "edmunds":     ("www.edmunds.com", "akamai"),
    "oracle":      ("www.oracle.com", "akamai"),
    "sap":         ("www.sap.com", "akamai"),
    "nytimes":     ("www.nytimes.com", "datadome"),
    "yelp":        ("www.yelp.com", "datadome"),
    "tripadvisor": ("www.tripadvisor.com", "datadome"),
    "amazon":      ("www.amazon.com", "awswaf"),
}

HIWAVE_UA = "Mozilla/5.0 (X11; Linux x86_64) HiWave/1.0 RustKit/1.0"

def rustkit_request(host):
    # byte-faithful to rustkit-http send_request()
    return ("GET / HTTP/1.1\r\n"
            f"Host: {host}\r\n"
            "User-Agent: RustKit/1.0\r\n"
            "Accept: */*\r\n"
            "Accept-Encoding: gzip\r\n"
            "Connection: close\r\n"
            "accept-language: en-US,en;q=0.9\r\n"
            "\r\n")

def hiwave_request(host):
    # browser-shaped: order/casing as Firefox/Chromium h1, honest UA
    return ("GET / HTTP/1.1\r\n"
            f"Host: {host}\r\n"
            f"User-Agent: {HIWAVE_UA}\r\n"
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8\r\n"
            "Accept-Language: en-US,en;q=0.9\r\n"
            "Accept-Encoding: gzip, deflate\r\n"
            "Connection: keep-alive\r\n"
            "Upgrade-Insecure-Requests: 1\r\n"
            "Sec-Fetch-Dest: document\r\n"
            "Sec-Fetch-Mode: navigate\r\n"
            "Sec-Fetch-Site: none\r\n"
            "Sec-Fetch-User: ?1\r\n"
            "\r\n")

def classify(status, body, headers, vendor):
    b = body[:4000].lower()
    h = str(headers).lower()
    marker = ""
    if "cf-mitigated" in h or "cf-chl" in b or "challenge-platform" in b: marker = "cf-challenge"
    elif "_abck" in h or "akamai" in b or "reference&#32;id" in b or "edgesuite" in b: marker = "akamai-denial"
    elif "datadome" in h or "datadome" in b: marker = "datadome"
    elif status == 202 and len(body.strip()) < 200: marker = "awswaf-202blank"
    ok = status == 200 and not marker
    return ("PASS" if ok else "BLOCK", f"{status}{'/'+marker if marker else ''}")

def fetch(host, req_bytes, alpn):
    ctx = ssl.create_default_context()
    if alpn: ctx.set_alpn_protocols(alpn)
    with socket.create_connection((host, 443), timeout=15) as sock:
        with ctx.wrap_socket(sock, server_hostname=host) as tls:
            sel = tls.selected_alpn_protocol()
            if sel == "h2":
                return None, None, None, "h2-selected"
            tls.sendall(req_bytes.encode())
            data = b""
            tls.settimeout(15)
            try:
                while len(data) < 65536:
                    chunk = tls.recv(8192)
                    if not chunk: break
                    data += chunk
            except (socket.timeout, ssl.SSLError):
                pass
    head, _, body = data.partition(b"\r\n\r\n")
    lines = head.decode("latin1").split("\r\n")
    status = int(lines[0].split()[1]) if lines and len(lines[0].split()) > 1 else 0
    return status, body.decode("latin1", "replace"), "\n".join(lines[1:]), None

def run():
    out = {}
    variants = [
        ("A_rustkit",      rustkit_request, None),
        ("B_rustkit_alpn", rustkit_request, ["http/1.1"]),
        ("C_hiwave",       hiwave_request,  ["http/1.1"]),
        ("D_hiwave_h2",    hiwave_request,  ["h2", "http/1.1"]),
    ]
    for site, (host, vendor) in SITES.items():
        row = {"vendor": vendor}
        for name, builder, alpn in variants:
            try:
                status, body, headers, note = fetch(host, builder(host), alpn)
                if note:
                    row[name] = note
                else:
                    verdict, detail = classify(status, body, headers, vendor)
                    row[name] = f"{verdict}:{detail}"
            except Exception as e:
                row[name] = f"ERR:{type(e).__name__}"
            time.sleep(0.4)
        out[site] = row
        print(f"{site:12} {row['vendor']:10} " +
              " ".join(f"{row.get(n[0],'-'):24}" for n in variants), flush=True)
    json.dump(out, open(sys.argv[1] if len(sys.argv) > 1 else "waf_probe.json", "w"), indent=1)

if __name__ == "__main__":
    run()
