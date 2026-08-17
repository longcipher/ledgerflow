# LedgerFlow Real-Network Tests

This directory contains scripts and artifacts for real on-chain tests of the
LedgerFlow payment protocols.

## Networks

| Network | Chain ID | RPC | Native | USDC ERC-20 |
|---|---|---|---|---|
| Arc Testnet | 5042002 | `https://rpc.testnet.arc.io` | USDC (18 dec gas) | `0x3600000000000000000000000000000000000000` (6 dec) |
| Tempo Testnet (Moderato) | 42431 | `https://rpc.moderato.tempo.xyz` | — | — |

## Test Wallet

- Address: `0x00B8E3e3d589577bAeAbbE0f72993e5F72e82f00`
- Imported in onecipher as `arc-test` (EVM).

## Tests

1. `arc-x402.sh` — real x402 (EIP-3009) payment on Arc Testnet.
2. MPP on Tempo Testnet — driven from the reference `mpp-rs` client/server
   suite (see `mpp-rs/tests/ledgerflow_moderato_live.rs`), using the same
   test wallet; the on-chain receipt is confirmed against
   `https://rpc.moderato.tempo.xyz`.
