#!/usr/bin/env python3
"""Brute-force common EIP-712 domain parameter combos to match on-chain
DOMAIN_SEPARATOR() for Arc USDC."""
import hashlib
import itertools

TARGET = "0x361191522483d32a83e70ae7183b4b9629442c13a78bc9921d6f707911c8c6b0"
CONTRACT = "0x3600000000000000000000000000000000000000"
CHAIN_IDS = [5042002, 1]
NAMES = ["USD Coin", "USDC", "Circle USD", "Circle USDC", ""]
VERSIONS = ["1", "2", "3"]
# salt variants: zero, contract, chain-scoped
SALTS = [
    "0x0000000000000000000000000000000000000000000000000000000000000000",
]

DOMAIN_TYPEHASH = hashlib.sha256(
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)"
).hexdigest()
# Standard domain without salt:
DOMAIN_TYPEHASH_NOSALT = hashlib.sha256(
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
).hexdigest()


def hstr(s):
    return hashlib.sha256(s.encode()).hexdigest()


def enc_addr(a):
    return "0x" + a.lower().replace("0x", "").rjust(64, "0")


def enc_uint(n):
    return hex(n).replace("0x", "").rjust(64, "0")


def sep_with_salt(name, ver, cid, contract, salt):
    packed = ("0x" + DOMAIN_TYPEHASH + hstr(name) + hstr(ver)
              + enc_uint(cid) + enc_addr(contract) + salt.replace("0x", "").rjust(64, "0"))
    return "0x" + hashlib.sha256(bytes.fromhex(packed.replace("0x", ""))).hexdigest()


def sep_no_salt(name, ver, cid, contract):
    packed = ("0x" + DOMAIN_TYPEHASH_NOSALT + hstr(name) + hstr(ver)
              + enc_uint(cid) + enc_addr(contract))
    return "0x" + hashlib.sha256(bytes.fromhex(packed.replace("0x", ""))).hexdigest()


found = []
for name, ver, cid in itertools.product(NAMES, VERSIONS, CHAIN_IDS):
    for salt in SALTS:
        s = sep_with_salt(name, ver, cid, CONTRACT, salt)
        if s == TARGET:
            found.append(("with-salt", name, ver, cid, salt))
    s = sep_no_salt(name, ver, cid, CONTRACT)
    if s == TARGET:
        found.append(("no-salt", name, ver, cid, "-"))

for f in found:
    print("FOUND:", f)
if not found:
    print("not found in common combos")
