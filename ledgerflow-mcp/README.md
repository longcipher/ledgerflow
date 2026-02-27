# ledgerflow-mcp

An MCP (Model Context Protocol) server that exposes x402 v2 verification and settlement tools for LedgerFlow using `ultrafast-mcp`.

## Tools

- x402_supported: fetch facilitator `/supported` (kinds/extensions/signers)
- x402_verify: forward an x402 v2 verify payload to facilitator `/verify`
- x402_settle: forward an x402 v2 settle payload to facilitator `/settle`

## Run

By default the server runs over stdio and proxies calls to `http://127.0.0.1:3402`.

```sh
cargo run -p ledgerflow-mcp -- --stdio
```

Override facilitator upstream URL:

```sh
cargo run -p ledgerflow-mcp -- --stdio --facilitator-url http://127.0.0.1:3402
```

To run as an HTTP server:

```sh
cargo run -p ledgerflow-mcp -- --http --host 127.0.0.1 --port 8765
```

Configure RPC endpoints and signer via env vars (same as `ledgerflow-facilitator`). See `ledgerflow-facilitator/README.md`.
