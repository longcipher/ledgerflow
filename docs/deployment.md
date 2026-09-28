# LedgerFlow Deployment Guide

## Overview

LedgerFlow can be deployed in two modes:

1. **Standalone**: Single-user self-hosted deployment
2. **SaaS**: Multi-tenant deployment behind a gateway

## Prerequisites

- Rust 1.85+ (for edition 2024)
- A valid Ed25519 issuer key (64 hex characters)
- (SaaS mode) A gateway that injects internal headers

## Standalone Deployment

### Quick Start

```bash
# Build the server
cargo build --release -p ledgerflow-server-bin

# Set the required issuer key
export LEDGERFLOW_ISSUER_KEY=<64-hex-ed25519-key>

# Run the server
./target/release/ledgerflow-server --revocation-store ./data/revocations.jsonl
```

### Configuration

| Variable | Default | Required | Description |
|---|---|---|---|
| `LEDGERFLOW_BIND` | `127.0.0.1:8080` | No | Listen address |
| `LEDGERFLOW_SAAS_MODE` | `standalone` | No | `standalone` or `saas` |
| `LEDGERFLOW_SERVICE_TOKEN` | — | Yes (saas) | Shared service token |
| `LEDGERFLOW_TENANT_ID` | `default` | No | Tenant id (standalone) |
| `LEDGERFLOW_ISSUER_KEY` | — | **Yes** | Hex Ed25519 issuer key |
| `LEDGERFLOW_WEBHOOK_URL` | — | No | Webhook delivery endpoint |

### Docker Deployment

```dockerfile
FROM rust:1.85-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p ledgerflow-server-bin

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ledgerflow-server /usr/local/bin/
COPY --from=builder /app/target/release/ledgerflow-cli /usr/local/bin/

ENV LEDGERFLOW_BIND=0.0.0.0:8080
EXPOSE 8080

ENTRYPOINT ["ledgerflow-server"]
```

```bash
# Build and run
docker build -t ledgerflow .
docker run -p 8080:8080 -e LEDGERFLOW_ISSUER_KEY=<key> ledgerflow
```

## SaaS Deployment

### Network Prerequisites

**Critical**: The `x-internal-*` headers trust relies on "only the gateway can reach the service". You MUST:

1. Deploy LedgerFlow behind a gateway (e.g., Kong, Envoy, AWS ALB)
2. Configure mTLS or network policies to ensure only the gateway can reach LedgerFlow
3. Never expose LedgerFlow directly to the public internet

### Gateway Configuration

The gateway must inject these headers:

| Header | Description |
|---|---|
| `x-internal-tenant-id` | Tenant identifier |
| `x-internal-user-id` | User identifier (optional) |
| `x-internal-roles` | Comma-separated roles (optional) |
| `x-internal-principal` | Principal identifier (optional) |
| `Authorization` | `Bearer <service-token>` |

### SaaS Configuration

```bash
export LEDGERFLOW_SAAS_MODE=saas
export LEDGERFLOW_SERVICE_TOKEN=<shared-secret>
export LEDGERFLOW_ISSUER_KEY=<64-hex-ed25519-key>
export LEDGERFLOW_BIND=0.0.0.0:8080
```

### Tenant Isolation

In SaaS mode, tenant isolation is enforced in three places:

1. **Warrant issuance keys / trust anchors**: Isolated per tenant
2. **Facilitator settlement accounts**: Isolated per tenant
3. **Admin data**: Filtered by `tenant_id`

## Production Checklist

### Security

- [ ] Use a strong, randomly generated issuer key
- [ ] Store the issuer key in a secrets manager (e.g., AWS Secrets Manager, HashiCorp Vault)
- [ ] Enable TLS termination at the gateway
- [ ] Configure mTLS between gateway and LedgerFlow (SaaS mode)
- [ ] Set up a persistent revocation store (file-based or database)
- [ ] Enable webhook delivery for audit events
- [ ] Configure log aggregation and monitoring

### Reliability

- [ ] Set up health check monitoring (`/healthz`)
- [ ] Configure graceful shutdown handling
- [ ] Set up backup for the revocation store
- [ ] Configure resource limits (CPU, memory)
- [ ] Set up alerting for verification failures

### Observability

- [ ] Enable tracing with OpenTelemetry
- [ ] Configure metrics export (Prometheus)
- [ ] Set up structured logging
- [ ] Configure webhook events for audit trail

## API Endpoints

| Endpoint | Method | Description |
|---|---|---|
| `/healthz` | GET | Liveness probe |
| `/v1/warrants` | POST | Issue a root warrant |
| `/v1/revocations` | POST | Revoke a warrant or holder |
| `/v1/settlements/{transaction_id}` | GET | Query settlement status |
| `/v1/audit` | GET | Get audit events |
| `/openapi.json` | GET | OpenAPI specification |
| `/swagger-ui` | GET | Swagger UI |

## Example: Issue a Warrant

```bash
curl -X POST http://localhost:8080/v1/warrants \
  -H "Content-Type: application/json" \
  -d '{
    "holder_public_key": "<32-byte-hex>",
    "merchant_id": "merchant-a",
    "amount_cap": 1000000,
    "ttl_secs": 86400
  }'
```

## Example: Revoke a Warrant

```bash
curl -X POST http://localhost:8080/v1/revocations \
  -H "Content-Type: application/json" \
  -d '{
    "warrant_id": "<16-byte-hex>"
  }'
```

## Troubleshooting

### Server fails to start

- Check that `LEDGERFLOW_ISSUER_KEY` is set and is a valid 64-character hex string
- Check that the revocation store path is writable
- Check that the bind address is not already in use

### Verification failures

- Ensure the warrant has not expired
- Ensure the warrant has not been revoked
- Check that the trusted issuer configuration matches the warrant issuer
- Verify the PoP freshness window (default 60 seconds + 30 seconds skew)

### Settlement failures

- Check that the payment subject is supported
- Verify the rail adapter configuration
- Check the revocation store for the warrant/holder
