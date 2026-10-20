#!/usr/bin/env python3
"""Local-only ClientHello dumper (no site traffic). Usage: hello_dump.py PORT > out.json ; accepts one connection."""
import socket, struct, json, sys
GREASE = {0x0a0a+0x1010*i for i in range(16)}
def g(v): return "GREASE" if v in GREASE else v
def parse(b):
    assert b[0] == 22
    p = 5; assert b[p] == 1
    p += 4; ver = struct.unpack(">H", b[p:p+2])[0]; p += 2 + 32
    sl = b[p]; p += 1 + sl
    cl = struct.unpack(">H", b[p:p+2])[0]; p += 2
    ciphers = [g(struct.unpack(">H", b[p+i:p+i+2])[0]) for i in range(0, cl, 2)]; p += cl
    cm = b[p]; p += 1 + cm
    el = struct.unpack(">H", b[p:p+2])[0]; p += 2
    end = p + el; exts = []; detail = {}
    while p < end:
        t, l = struct.unpack(">HH", b[p:p+4]); body = b[p+4:p+4+l]; p += 4 + l
        exts.append(g(t))
        u16s = lambda x: [g(struct.unpack(">H", x[i:i+2])[0]) for i in range(0, len(x), 2)]
        if t == 10: detail["groups"] = u16s(body[2:])
        elif t == 13: detail["sigalgs"] = [hex(v) for v in u16s(body[2:])]
        elif t == 43: detail["versions"] = u16s(body[1:])
        elif t == 16:
            q = 2; a = []
            while q < len(body): n = body[q]; a.append(body[q+1:q+1+n].decode()); q += 1 + n
            detail["alpn"] = a
        elif t == 27: detail["cert_compression"] = u16s(body[1:])
        elif t in (17513, 17613): detail["alps_ext"] = [t, body.hex()]
        elif t == 45: detail["psk_modes"] = list(body[1:])
        elif t == 11: detail["ec_point_formats"] = list(body[1:])
        elif t == 51:
            q = 2; ks = []
            while q < len(body):
                grp, n = struct.unpack(">HH", body[q:q+4]); ks.append([g(grp), n]); q += 4 + n
            detail["key_shares"] = ks
    return {"record_version": hex(struct.unpack(">H", b[1:3])[0]), "client_version": hex(ver), "ciphers": ciphers,
            "extensions_in_order": exts, "extensions_sorted": sorted(map(str, exts)), **detail}
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen(1)
c, _ = s.accept(); c.settimeout(3)
data = b""
while len(data) < 5 or len(data) < 5 + struct.unpack(">H", data[3:5])[0]:
    d = c.recv(65536)
    if not d: break
    data += d
print(json.dumps(parse(data), indent=1)); c.close()
