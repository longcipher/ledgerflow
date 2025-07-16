#!/bin/bash

# 简单演示脚本 - 预测合约地址
# Simple demonstration script - Predict contract addresses

echo "=== PaymentVault 跨链地址预测演示 ==="
echo "=== PaymentVault Cross-Chain Address Prediction Demo ==="
echo ""

# 设置演示参数
export DEPLOYER_ADDRESS="0x742d35Cc6231A123456789012345678901234567"
export USDC_TOKEN_ADDRESS="0xA0b86a33E6417c5DeF6Ca95E2B6b81b9c8C06b6"
export INITIAL_OWNER="0x742d35Cc6231A123456789012345678901234567"

echo "演示参数 (Demo Parameters):"
echo "部署者地址 (Deployer): $DEPLOYER_ADDRESS"
echo "USDC 代币地址 (USDC Token): $USDC_TOKEN_ADDRESS"
echo "初始所有者 (Initial Owner): $INITIAL_OWNER"
echo ""

echo "运行地址预测脚本..."
echo "Running address prediction script..."
echo ""

forge script script/PredictAddresses.s.sol

echo ""
echo "=== 演示完成 ==="
echo "=== Demo Complete ==="
echo ""
echo "注意: 相同的地址将在所有 EVM 兼容链上生成"
echo "Note: The same addresses will be generated on all EVM-compatible chains"
echo ""
echo "要进行实际部署，请设置您的私钥并运行:"
echo "To perform actual deployment, set your private key and run:"
echo "export PRIVATE_KEY=0x..."
echo "./deploy_multichain.sh"
