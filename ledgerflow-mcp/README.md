# ledgerflow-mcp

An MCP (Model Context Protocol) server that exposes x402 verification and settlement tools for LedgerFlow using `ultrafast-mcp`.

## Tools

- x402_supported: list supported networks and schemes
- x402_verify: verify a payment intent (Exact scheme)
- x402_settle: settle a verified intent (Exact scheme)

## Run

By default the server runs over stdio.

```sh
cargo run -p ledgerflow-mcp -- --stdio
```

To run as an HTTP server:

```sh
cargo run -p ledgerflow-mcp -- --http --host 127.0.0.1 --port 8765
```

Configure RPC endpoints and signer via env vars (same as `ledgerflow-facilitator`). See `ledgerflow-facilitator/README.md`.
