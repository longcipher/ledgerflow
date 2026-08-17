#!/usr/bin/env python3
"""Decode an eip712Domain() ABI-encoded return value."""
import sys

data = sys.argv[1]
if data.startswith("0x"):
    data = data[2:]
b = bytes.fromhex(data)
words = [b[i:i+32] for i in range(0, len(b), 32)]

def word_to_int(w):
    return int.from_bytes(w, "big")

# eip712Domain returns: (bytes1 fields, string name, string version, uint256 chainId,
#                       address verifyingContract, bytes32 salt, uint256[] extensions)
i = 0
fields = words[i][31]  # bytes1
i += 1

def read_dynamic(words, i):
    offset = word_to_int(words[i])
    i += 1
    length = word_to_int(words[offset // 32])
    start = offset // 32 + 1
    raw = b"".join(words[start:start + (length + 31) // 32])
    return raw[:length].decode("utf-8", "replace"), offset // 32 + (length + 31) // 32

# name at offset
name, i = read_dynamic(words, i)
version, i = read_dynamic(words, i)
chain_id = word_to_int(words[i]); i += 1
verifying = "0x" + words[i][12:].hex(); i += 1
salt = "0x" + words[i].hex(); i += 1

print(f"fields(bytes1) = 0x{fields:02x}")
print(f"name          = {name!r}")
print(f"version       = {version!r}")
print(f"chainId       = {chain_id}")
print(f"verifying     = {verifying}")
print(f"salt          = {salt}")
