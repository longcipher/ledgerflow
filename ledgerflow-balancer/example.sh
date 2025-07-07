#!/bin/bash

# LedgerFlow Balancer API Example Script

BASE_URL="http://localhost:3000"

echo "=== LedgerFlow Balancer API Examples ==="

# Check if service is running
echo "1. Checking service health..."
curl -s "$BASE_URL/health" | jq .

echo -e "\n2. Creating a new order..."
ORDER_RESPONSE=$(curl -s -X POST "$BASE_URL/orders" \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "telegram_123456",
    "amount": "10.50",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000"
  }')

echo "$ORDER_RESPONSE" | jq .

# Extract order_id for subsequent requests
ORDER_ID=$(echo "$ORDER_RESPONSE" | jq -r '.order_id')
ACCOUNT_ID="telegram_123456"

echo -e "\n3. Getting order details..."
curl -s "$BASE_URL/orders/$ORDER_ID" | jq .

echo -e "\n4. Getting account balance..."
curl -s "$BASE_URL/accounts/$ACCOUNT_ID/balance" | jq .

echo -e "\n5. Creating another order for the same account..."
curl -s -X POST "$BASE_URL/orders" \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "telegram_123456",
    "amount": "5.25",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000"
  }' | jq .

echo -e "\n6. Trying to create a third order (should fail due to limit)..."
curl -s -X POST "$BASE_URL/orders" \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "telegram_123456",
    "amount": "1.00",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000"
  }' | jq .

echo -e "\n7. Listing pending orders (admin)..."
curl -s "$BASE_URL/admin/orders?limit=10&offset=0" | jq .

echo -e "\n8. Testing with different account..."
curl -s -X POST "$BASE_URL/orders" \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "telegram_789012",
    "amount": "25.00",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000"
  }' | jq .

echo -e "\n9. Getting balance for new account..."
curl -s "$BASE_URL/accounts/telegram_789012/balance" | jq .

echo -e "\n=== End of Examples ==="
