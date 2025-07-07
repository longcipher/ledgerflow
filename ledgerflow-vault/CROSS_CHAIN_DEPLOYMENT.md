# 跨链部署指南 (Cross-Chain Deployment Guide)

本指南介绍如何使用 CREATE2 技术在多条 EVM 兼容链上部署相同地址的 LedgerFlow Vault 合约。

## 概述

通过使用 CREATE2 操作码，我们可以在不同的 EVM 兼容链上部署具有相同地址的合约。这需要：

1. 相同的部署者地址
2. 相同的 salt 值
3. 相同的合约字节码
4. 相同的构造参数

## 文件结构

```
script/
├── DeployDeterministic.s.sol    # CREATE2 部署脚本
├── PredictAddresses.s.sol       # 地址预测脚本
└── DeployUpgradeable.s.sol      # 原始部署脚本（单链）

deploy_multichain.sh             # 多链部署自动化脚本
chain_config.env                 # 链配置文件
foundry.toml                     # Foundry 配置（包含多链 RPC）
```

## 使用步骤

### 1. 环境准备

首先设置必要的环境变量：

```bash
# 必需：私钥
export PRIVATE_KEY="0x您的私钥"

# 可选：初始所有者地址（默认使用部署者地址）
export INITIAL_OWNER="0x您的所有者地址"

# 可选：用于地址预测的部署者地址
export DEPLOYER_ADDRESS="0x您的部署者地址"
```

### 2. 地址预测

在实际部署前，可以预测合约地址：

```bash
# 预测所有链上的合约地址
forge script script/PredictAddresses.s.sol

# 或者指定特定参数预测
DEPLOYER_ADDRESS=0x您的地址 \
USDC_TOKEN_ADDRESS=0xUSDC地址 \
INITIAL_OWNER=0x所有者地址 \
forge script script/PredictAddresses.s.sol
```

### 3. 单链部署

部署到单条链（以 Sepolia 为例）：

```bash
# 设置 USDC 地址
export USDC_TOKEN_ADDRESS="0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"

# 部署到 Sepolia
forge script script/DeployDeterministic.s.sol \
  --rpc-url sepolia \
  --private-key $PRIVATE_KEY \
  --broadcast \
  --verify
```

### 4. 多链自动化部署

使用自动化脚本部署到多条链：

```bash
# 给脚本添加执行权限（如果还没有）
chmod +x deploy_multichain.sh

# 运行多链部署
./deploy_multichain.sh
```

### 5. 自定义链部署

如果要部署到脚本中未包含的链，可以手动执行：

```bash
# 设置链特定的环境变量
export USDC_TOKEN_ADDRESS="0x链特定的USDC地址"

# 部署到自定义 RPC
forge script script/DeployDeterministic.s.sol \
  --rpc-url "https://your-custom-rpc-url" \
  --private-key $PRIVATE_KEY \
  --broadcast \
  --verify
```

## 支持的链

### 测试网（推荐先在测试网验证）

- **Sepolia** (Ethereum)
- **Polygon Mumbai** 
- **Arbitrum Sepolia**
- **Optimism Sepolia**
- **Base Sepolia**
- **Avalanche Fuji**
- **BSC Testnet**
- **Unichain Sepolia**

### 主网

- **Ethereum**
- **Polygon**
- **Arbitrum**
- **Optimism**
- **Base**
- **Avalanche**
- **BSC**

## USDC 地址配置

不同链上的 USDC 合约地址不同，请确保使用正确的地址：

### 主网 USDC 地址
- Ethereum: `0xA0b86a33E6417c5DeF6Ca95E2B6b81b9c8C06b6`
- Polygon: `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`
- Arbitrum: `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`
- Optimism: `0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85`
- Base: `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`

### 测试网 USDC 地址
请参考 `chain_config.env` 文件中的地址配置。

## 验证部署

部署完成后，验证所有链上的合约地址是否相同：

```bash
# 检查合约地址
forge script script/PredictAddresses.s.sol

# 在区块链浏览器中验证:
# - 合约地址是否一致
# - 合约代码是否正确验证
# - 初始化参数是否正确
```

## 安全注意事项

1. **私钥安全**: 确保私钥安全存储，建议使用硬件钱包或多签钱包
2. **测试优先**: 先在测试网部署和测试，确认无误后再部署主网
3. **地址验证**: 部署前后都要验证 USDC 地址和其他参数的正确性
4. **Gas 费用**: 准备足够的 gas 费用在各个链上进行部署
5. **Nonce 同步**: 确保部署者账户在所有链上的 nonce 状态一致

## 故障排除

### 地址不一致问题
- 检查是否使用了相同的 salt 值
- 确认部署者地址是否一致
- 验证构造参数是否相同

### 部署失败问题
- 检查 gas 费用是否充足
- 确认 RPC 连接是否正常
- 验证私钥和权限设置

### 验证失败问题
- 检查 Etherscan API key 是否正确
- 确认链的验证服务是否可用
- 手动在区块链浏览器上验证

## 升级说明

由于使用了 UUPS 代理模式，后续升级只需：

1. 部署新的实现合约
2. 调用代理合约的升级函数
3. 新的实现合约可以不需要相同地址

## 联系信息

如有问题，请参考：
- Foundry 文档：https://book.getfoundry.sh/
- OpenZeppelin 代理文档：https://docs.openzeppelin.com/contracts/4.x/upgradeable
