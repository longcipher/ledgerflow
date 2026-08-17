#!/usr/bin/env python3
"""Compute EIP-712 domain separator for Arc USDC and compare with on-chain.

Usage: python3 compute_domain.py [--name NAME --version VER --chain-id N --contract ADDR]
"""
import hashlib
import sys

DOMAIN_TYPEHASH = hashlib.sha256(
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)"
).hexdigest()


def hash_string(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def encode_address(addr: str) -> str:
    return "0x" + addr.lower().replace("0x", "").rjust(64, "0")


def encode_uint(n: int) -> str:
    return hex(n).replace("0x", "").rjust(64, "0")


def domain_separator(name, version, chain_id, contract, salt) -> str:
    packed = (
        "0x" + DOMAIN_TYPEHASH
        + hash_string(name)
        + hash_string(version)
        + encode_uint(chain_id)
        + encode_address(contract)
        + salt.replace("0x", "").lower().rjust(64, "0")
    )
    raw = bytes.fromhex(packed.replace("0x", ""))
    return "0x" + hashlib.sha256(raw).hexdigest()


if __name__ == "__main__":
    name = sys.argv[sys.argv.index("--name") + 1] if "--name" in sys.argv else "USD Coin"
    version = sys.argv[sys.argv.index("--version") + 1] if "--version" in sys.argv else "2"
    chain_id = int(sys.argv[sys.argv.index("--chain-id") + 1]) if "--chain-id" in sys.argv else 5042002
    contract = sys.argv[sys.argv.index("--contract") + 1] if "--contract" in sys.argv else "0x3600000000000000000000000000000000000000"
    salt = sys.argv[sys.argv.index("--salt") + 1] if "--salt" in sys.argv else "0x0000000000000000000000000000000000000000000000000000000000000000"
    print(domain_separator(name, version, chain_id, contract, salt))
