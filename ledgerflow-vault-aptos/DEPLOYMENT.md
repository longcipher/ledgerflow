# LedgerFlow Vault Aptos - 部署文档

## 部署信息

### 网络环境
- **网络**: Aptos Testnet
- **部署时间**: 2025年7月19日

### 合约地址
- **合约地址**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846`
- **模块名称**: `payment_vault_fa`
- **完整标识**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa`

### USDC 配置
- **USDC 元数据地址**: `0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832`
- **USDC 类型**: Fungible Asset (FA)
- **精度**: 6 位小数

### 部署交易
- **发布交易**: [`0x4e72687e72fd0de0fbe91991cabc9a801769920845558630ee50c9b9bda2a74c`](https://explorer.aptoslabs.com/txn/0x4e72687e72fd0de0fbe91991cabc9a801769920845558630ee50c9b9bda2a74c?network=testnet)
- **初始化交易**: [`0xb8505febe79ae6eea9bc6d6c0d95b9f87705df878fb809834fa3cc39690d515b`](https://explorer.aptoslabs.com/txn/0xb8505febe79ae6eea9bc6d6c0d95b9f87705df878fb809834fa3cc39690d515b?network=testnet)

### 当前状态
- **合约状态**: ✅ 已初始化
- **拥有者**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846`
- **当前余额**: 0 USDC
- **存款计数**: 0

## 可用功能

### 1. 存款功能
```bash
aptos move run --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::deposit \
  --args \
    address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846 \
    string:"order_id_123456" \
    u64:1000000
```

### 2. 提款功能（仅限拥有者）
```bash
aptos move run --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::withdraw \
  --args \
    address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846 \
    address:RECIPIENT_ADDRESS \
    u64:AMOUNT
```

### 3. 查询功能

#### 检查合约状态
```bash
aptos move view --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::vault_exists \
  --args address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846
```

#### 查询余额
```bash
aptos move view --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::get_balance \
  --args address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846
```

#### 查询拥有者
```bash
aptos move view --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::get_owner \
  --args address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846
```

#### 查询存款次数
```bash
aptos move view --function-id 0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846::payment_vault_fa::get_deposit_count \
  --args address:0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846
```

## 技术特性

### 安全特性
- ✅ 使用 Fungible Asset 标准
- ✅ 基于能力的访问控制
- ✅ 原子操作保证
- ✅ 输入验证
- ✅ 事件发射用于监控

### 事件系统
- `DepositReceived`: 存款完成事件
- `WithdrawCompleted`: 提款完成事件
- `OwnershipTransferred`: 所有权转移事件

### 兼容性
- ✅ 支持 Aptos Fungible Asset 标准
- ✅ 与 Circle USDC 兼容
- ✅ 支持原生钱包集成

## 注意事项

1. **USDC 余额**: 用户需要有足够的 USDC 余额才能进行存款
2. **Gas 费用**: 所有交易都需要支付 APT 作为 Gas 费用
3. **权限控制**: 只有合约拥有者可以进行提款操作
4. **订单 ID**: 存款时需要提供唯一的订单 ID

## 后续开发

- [ ] 添加多签名支持
- [ ] 实现紧急停止功能
- [ ] 添加费用收取机制
- [ ] 集成价格预言机
- [ ] 添加批量操作支持

## 联系信息

- **项目**: LedgerFlow
- **开发者**: LongCipher
- **GitHub**: https://github.com/longcipher/ledgerflow
