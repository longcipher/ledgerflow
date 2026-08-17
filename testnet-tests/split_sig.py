#!/usr/bin/env python3
"""Split an EIP-3009 signature (64 hex bytes from onecipher/cast) into
r (bytes32), s (bytes32), v (uint8 27/28).

Usage: python3 split_sig.py <sig-hex> [--json]
"""
import sys

sig = sys.argv[1]
sig = sig[2:] if sig.startswith("0x") else sig
assert len(sig) >= 128, f"sig too short: {len(sig)}"

r = "0x" + sig[:64]
s = "0x" + sig[64:128]
# v is the last byte (27 or 28); cast/onecipher return r||s||v hex
tail = sig[128:]
v_byte = int(tail, 16) if tail else 0
v = v_byte if v_byte in (27, 28) else v_byte + 27

if "--json" in sys.argv:
    import json
    print(json.dumps({"r": r, "s": s, "v": v}))
else:
    print(f"r={r}")
    print(f"s={s}")
    print(f"v={v}")
