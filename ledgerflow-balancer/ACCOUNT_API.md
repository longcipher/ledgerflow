# LedgerFlow Balancer - Account Management API Documentation

## Overview

The LedgerFlow Balancer now includes comprehensive account management functionality, allowing users to register accounts and query account information through various identifiers.

## New API Endpoints

### 1. Register Account

**Endpoint:** `POST /register`

**Description:** Creates a new account with the provided information.

**Request Body:**
```json
{
  "username": "string (required)",
  "email": "string (optional)",
  "telegram_id": "integer (optional)",
  "evm_address": "string (optional)"
}
```

**Response:**
```json
{
  "id": 123,
  "username": "john_doe",
  "email": "john@example.com",
  "telegram_id": 123456789,
  "evm_address": "0x1234567890abcdef1234567890abcdef12345678",
  "created_at": "2025-07-09T10:30:00Z",
  "updated_at": "2025-07-09T10:30:00Z"
}
```

**Error Responses:**
- `400 Bad Request`: Invalid input or duplicate username/email/telegram_id
- `500 Internal Server Error`: Database error

### 2. Get Account by Username

**Endpoint:** `GET /accounts/username/{username}`

**Description:** Retrieves account information by username.

**Path Parameters:**
- `username`: The username to search for

**Response:**
```json
{
  "id": 123,
  "username": "john_doe",
  "email": "john@example.com",
  "telegram_id": 123456789,
  "evm_address": "0x1234567890abcdef1234567890abcdef12345678",
  "created_at": "2025-07-09T10:30:00Z",
  "updated_at": "2025-07-09T10:30:00Z"
}
```

**Error Responses:**
- `404 Not Found`: Account with the specified username not found
- `500 Internal Server Error`: Database error

### 3. Get Account by Email

**Endpoint:** `GET /accounts/email/{email}`

**Description:** Retrieves account information by email address.

**Path Parameters:**
- `email`: The email address to search for

**Response:** Same as above

**Error Responses:**
- `404 Not Found`: Account with the specified email not found
- `500 Internal Server Error`: Database error

### 4. Get Account by Telegram ID

**Endpoint:** `GET /accounts/telegram/{telegram_id}`

**Description:** Retrieves account information by Telegram ID.

**Path Parameters:**
- `telegram_id`: The Telegram ID to search for (integer)

**Response:** Same as above

**Error Responses:**
- `404 Not Found`: Account with the specified Telegram ID not found
- `500 Internal Server Error`: Database error

## Database Schema

The `accounts` table has been updated to support the new functionality:

```sql
CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    telegram_id BIGINT UNIQUE,
    email VARCHAR(320),
    evm_address VARCHAR(42),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

## Business Rules

1. **Username Uniqueness**: Each username must be unique across all accounts
2. **Email Uniqueness**: Each email (if provided) must be unique across all accounts
3. **Telegram ID Uniqueness**: Each Telegram ID (if provided) must be unique across all accounts
4. **Required Fields**: Only `username` is required; all other fields are optional
5. **EVM Address Format**: If provided, EVM addresses should be 42 characters long (0x + 40 hex characters)

## Usage Examples

### Register a new account with all fields:
```bash
curl -X POST http://localhost:3000/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice_crypto",
    "email": "alice@example.com",
    "telegram_id": 987654321,
    "evm_address": "0xabcdef1234567890abcdef1234567890abcdef12"
  }'
```

### Register with minimal information:
```bash
curl -X POST http://localhost:3000/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob_minimal"
  }'
```

### Query account by username:
```bash
curl http://localhost:3000/accounts/username/alice_crypto
```

### Query account by email:
```bash
curl http://localhost:3000/accounts/email/alice@example.com
```

### Query account by Telegram ID:
```bash
curl http://localhost:3000/accounts/telegram/987654321
```

## Integration with Other Services

The account management functionality integrates seamlessly with the existing order management system:

1. **Order Creation**: Orders are still created using the account ID (primary key)
2. **Balance Queries**: Account balances are retrieved using the account ID
3. **Cross-Reference**: You can now easily find an account ID using username, email, or Telegram ID

## Error Handling

All endpoints return consistent error responses:

```json
{
  "error": "Error type",
  "message": "Detailed error message",
  "status": 400
}
```

Common error scenarios:
- Duplicate username/email/telegram_id during registration
- Account not found during queries
- Invalid input format
- Database connection issues

## Testing

A test script is provided (`test_api.sh`) that demonstrates all the new functionality:

```bash
cd /path/to/ledgerflow-balancer
./test_api.sh
```

This script will test account registration, all query methods, and error handling scenarios.
