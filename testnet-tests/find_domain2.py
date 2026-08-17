#!/usr/bin/env python3
"""Extended brute-force for Arc USDC EIP-712 domain separator."""
import hashlib
import itertools

TARGET = "0x361191522483d32a83e70ae7183b4b9629442c13a78bc9921d6f707911c8c6b0"
CONTRACT = "0x3600000000000000000000000000000000000000"

NAMES = ["USDC", "USD Coin", "Circle USD", "Circle USDC", ""]
VERSIONS = ["1", "2", "3", ""]
CHAIN_IDS = [5042002, 0, 1, 8453, 137, 42161]

# domain typehash variants (with/without salt)
TYPEHASHES = {
    "with-salt": b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)",
    "no-salt": b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    "just-name-ver": b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
}


def hstr(s):
    return hashlib.sha256(s.encode()).hexdigest()


def enc_addr(a):
    return "0x" + a.lower().replace("0x", "").rjust(64, "0")


def enc_uint(n):
    return hex(n).replace("0x", "").rjust(64, "0")


def enc_bytes32(hexstr):
    return hexstr.replace("0x", "").lower().rjust(64, "0")


found = []
for name, ver, cid in itertools.product(NAMES, VERSIONS, CHAIN_IDS):
    for label, th in TYPEHASHES.items():
        typehash = hashlib.sha256(th).hexdigest()
        parts = ["0x" + typehash, hstr(name), hstr(ver), enc_uint(cid), enc_addr(CONTRACT)]
        if label == "with-salt":
            parts.append(enc_bytes32("0x" + "00" * 32))
        packed = "".join(parts)
        s = "0x" + hashlib.sha256(bytes.fromhex(packed.replace("0x", ""))).hexdigest()
        if s == TARGET:
            found.append((label, repr(name), repr(ver), cid))

for f in found:
    print("FOUND:", f)
if not found:
    print("still not found")
