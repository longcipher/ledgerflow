#!/usr/bin/env python3
"""Verify an EIP-712 signature by recovering the signer address (pure python).

Usage:
  python3 verify_eip712.py <typed-data.json> <signature-hex> <expected-address>
"""
import hashlib
import json
import sys

try:
    from eth_keys import keys  # noqa
    HAS_ETH_KEYS = True
except ImportError:
    HAS_ETH_KEYS = False

try:
    import coincurve
    HAS_COINCURVE = True
except ImportError:
    HAS_COINCURVE = False


def h(s):
    return hashlib.sha256(s.encode()).hexdigest()


def ea(a):
    return a[2:].lower().rjust(64, "0")


def eu(n):
    return hex(int(n))[2:].rjust(64, "0")


def e32(x):
    return x[2:].lower().rjust(64, "0")


def ebytes(x):
    if isinstance(x, str) and x.startswith("0x"):
        return x[2:].lower()
    return x.encode().hex()


def hash_struct(primary, types, message):
    def resolve(t):
        return f"{t}({','.join(f['type'] + ' ' + f['name'] for f in types[t])})"

    deps = []

    def collect(t):
        for f in types.get(t, []):
            ft = f["type"]
            if ft in types and ft not in deps and ft != "EIP712Domain":
                collect(ft)
                deps.append(ft)

    collect(primary)
    full_type = resolve(primary) + "".join(resolve(d) for d in deps)
    th = hashlib.sha256(full_type.encode()).hexdigest()

    def enc(field):
        ft = field["type"]
        v = message[field["name"]]
        if ft == "address":
            return ea(v)
        if ft.startswith("uint"):
            return eu(v)
        if ft == "bytes32":
            return e32(v)
        if ft == "string":
            return h(v)
        return ebytes(v)

    encs = [th] + [enc(f) for f in types[primary]]
    return hashlib.sha256(bytes.fromhex("".join(encs))).hexdigest()


def domain_separator(domain):
    th = hashlib.sha256(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    ).hexdigest()
    parts = [th, h(domain["name"]), h(domain["version"]), eu(domain["chainId"]), ea(domain["verifyingContract"])]
    return hashlib.sha256(bytes.fromhex("".join(parts))).hexdigest()


def main():
    td_path, sig_hex, expected = sys.argv[1], sys.argv[2], sys.argv[3]
    td = json.load(open(td_path))
    ds = domain_separator(td["domain"])
    sh = hash_struct(td["primaryType"], td["types"], td["message"])
    digest = hashlib.sha256(b"\x19\x01" + bytes.fromhex(ds) + bytes.fromhex(sh)).hexdigest()
    print("domain_separator:", ds)
    print("struct_hash:     ", sh)
    print("digest:          ", digest)

    sig = sig_hex[2:] if sig_hex.startswith("0x") else sig_hex
    r = int(sig[:64], 16)
    s = int(sig[64:128], 16)
    v = int(sig[128:130], 16)
    if v not in (27, 28):
        v += 27

    recovered = None
    if HAS_COINCURVE:
        import coincurve
        pub = coincurve.PublicKey.from_valid_signature_and_message(
            bytes.fromhex(sig[:128]),
            bytes.fromhex(digest),
            hasher=None,
        )
        recovered = "0x" + hashlib.sha256(pub.format(compressed=False)[1:]).hexdigest()[-40:]
    elif HAS_ETH_KEYS:
        from eth_keys import keys as ek
        sig_obj = ek.Signature(bytes.fromhex(sig[:128]))
        pk = sig_obj.recover_public_key_from_msg_hash(bytes.fromhex(digest))
        recovered = pk.to_checksum_address()
    else:
        print("no secp256k1 library available; install eth-keys or coincurve")
        sys.exit(2)

    print("recovered:       ", recovered.lower())
    print("expected:        ", expected.lower())
    ok = recovered.lower() == expected.lower()
    print("MATCH" if ok else "MISMATCH")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
