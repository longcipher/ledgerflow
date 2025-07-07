# UUPS 部署与升级命令总结

## 新增文件

### 合约文件
- `src/PaymentVault.sol` - 已修改为支持UUPS升级的主合约
- `test/PaymentVaultUpgrade.t.sol` - 升级功能测试

### 部署脚本
- `script/DeployUpgradeable.s.sol` - UUPS代理部署脚本
- `script/UpgradePaymentVault.s.sol` - 合约升级脚本

### 文档
- `UUPS_UPGRADE.md` - 详细的UUPS升级文档
- `README.md` - 已更新包含UUPS部署命令

## 部署命令

### 1. 初始部署（使用代理模式）

```bash
# 设置环境变量
export PRIVATE_KEY=your_private_key
export RPC_URL=https://sepolia.unichain.org

# 部署到Unichain Sepolia
forge script script/DeployUpgradeable.s.sol --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast --verify

# 或部署到其他网络
forge script script/DeployUpgradeable.s.sol --rpc-url <RPC_URL> --private-key <PRIVATE_KEY> --broadcast
```

**输出信息：**
- Implementation address: 实现合约地址（每次升级都会变化）
- Proxy address: 代理合约地址（用户交互的固定地址）
- Owner: 合约所有者
- USDC Token: 配置的USDC代币地址

### 2. 升级合约

```bash
# 1. 编辑 script/UpgradePaymentVault.s.sol 设置正确的代理地址
# 2. 运行升级脚本
forge script script/UpgradePaymentVault.s.sol --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast
```

## 测试命令

### 基础测试
```bash
# 运行所有测试
forge test

# 运行升级相关测试
forge test --match-contract PaymentVaultUpgradeTest

# 带gas报告的测试
forge test --match-contract PaymentVaultUpgradeTest --gas-report
```

### 编译检查
```bash
# 编译所有合约
forge build

# 检查代码格式
forge fmt
```

## 重要注意事项

### 地址说明
- **实现地址（Implementation Address）**: 逻辑合约地址，每次升级都会变化
- **代理地址（Proxy Address）**: 用户交互的永久地址，永远不变
- **用户应始终使用代理地址进行交互**

### 升级安全检查
1. ✅ 确保只有合约所有者可以授权升级
2. ✅ 在测试网上充分测试升级
3. ✅ 验证存储布局兼容性
4. ✅ 检查所有状态变量是否保留
5. ✅ 考虑为额外安全性添加升级时间锁

### 存储布局兼容性规则
创建新版本（V2, V3等）时：
- ✅ 在末尾添加新的状态变量
- ✅ 添加新函数
- ❌ 删除现有状态变量
- ❌ 更改现有状态变量的顺序
- ❌ 更改现有状态变量的类型

## 快速命令参考

```bash
# 构建
make build

# 测试
make test

# 部署标准版本
make deploy-unichain-sepolia

# 部署可升级版本
forge script script/DeployUpgradeable.s.sol --rpc-url https://sepolia.unichain.org --private-key $PRIVATE_KEY --broadcast

# 测试升级功能
forge test --match-contract PaymentVaultUpgradeTest

# 验证合约
make verify-unichain-sepolia
```
