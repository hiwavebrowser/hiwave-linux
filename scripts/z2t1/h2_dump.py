#!/usr/bin/env python3
"""Local-only h2 client-frame dumper: TLS (ALPN h2) server, decodes the client's first frames. Usage: h2_dump.py PORT CERT KEY"""
import socket, ssl, struct, sys, json
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); ctx.load_cert_chain(sys.argv[2], sys.argv[3]); ctx.set_alpn_protocols(["h2"])
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen(1)
c, _ = s.accept(); c = ctx.wrap_socket(c, server_side=True); c.settimeout(2)
buf = b""
try:
    while True:
        d = c.recv(65536)
        if not d: break
        buf += d
        if len(buf) > 24 and b"\x01" in buf and len(buf) > 200: pass
except Exception: pass
out = {"preface_ok": buf[:24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", "frames": []}
p = 24
names = {0:"DATA",1:"HEADERS",2:"PRIORITY",3:"RST",4:"SETTINGS",6:"PING",7:"GOAWAY",8:"WINDOW_UPDATE"}
while p + 9 <= len(buf):
    ln = int.from_bytes(buf[p:p+3], "big"); t = buf[p+3]; fl = buf[p+4]; sid = struct.unpack(">I", buf[p+5:p+9])[0] & 0x7fffffff
    body = buf[p+9:p+9+ln]; f = {"type": names.get(t, t), "flags": hex(fl), "stream": sid, "len": ln}
    if t == 4: f["settings"] = [[struct.unpack(">H", body[i:i+2])[0], struct.unpack(">I", body[i+2:i+6])[0]] for i in range(0, len(body), 6)]
    if t == 8: f["increment"] = struct.unpack(">I", body[:4])[0]
    if t == 1 and fl & 0x20: f["priority"] = {"exclusive": body[0] >> 7, "dep": struct.unpack(">I", body[:4])[0] & 0x7fffffff, "weight": body[4] + 1}
    if t == 1: f["hpack_hex"] = body.hex()
    out["frames"].append(f); p += 9 + ln
print(json.dumps(out, indent=1))
