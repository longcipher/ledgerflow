#!/bin/bash

# Test script for the new account registration and query APIs

BASE_URL="http://localhost:3000"

echo "Testing LedgerFlow Balancer Account APIs"
echo "======================================="

# Test 1: Register a new account
echo "1. Testing account registration..."
curl -X POST "$BASE_URL/register" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "test_user",
    "email": "test@example.com",
    "telegram_id": 123456789,
    "evm_address": "0x1234567890abcdef1234567890abcdef12345678"
  }' \
  -w "\nStatus: %{http_code}\n\n"

# Test 2: Get account by username
echo "2. Testing get account by username..."
curl -X GET "$BASE_URL/accounts/username/test_user" \
  -w "\nStatus: %{http_code}\n\n"

# Test 3: Get account by email
echo "3. Testing get account by email..."
curl -X GET "$BASE_URL/accounts/email/test@example.com" \
  -w "\nStatus: %{http_code}\n\n"

# Test 4: Get account by telegram ID
echo "4. Testing get account by telegram ID..."
curl -X GET "$BASE_URL/accounts/telegram/123456789" \
  -w "\nStatus: %{http_code}\n\n"

# Test 5: Test non-existent account
echo "5. Testing non-existent account (should return 404)..."
curl -X GET "$BASE_URL/accounts/username/non_existent_user" \
  -w "\nStatus: %{http_code}\n\n"

# Test 6: Register another account with minimal data
echo "6. Testing account registration with minimal data..."
curl -X POST "$BASE_URL/register" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "minimal_user"
  }' \
  -w "\nStatus: %{http_code}\n\n"

# Test 7: Test duplicate username (should fail)
echo "7. Testing duplicate username registration (should fail)..."
curl -X POST "$BASE_URL/register" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "test_user",
    "email": "another@example.com"
  }' \
  -w "\nStatus: %{http_code}\n\n"

echo "Test completed!"
