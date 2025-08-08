# LedgerFlow Vault Permit 功能使用指南

## 概述

`depositWithPermit` 方法允许用户通过离线签名完成代币批准和存款操作，在单一交易中完成，从而节省燃气费用。

## 功能特点

- **燃气效率**: 将批准 (approve) 和存款操作合并为单一交易
- **离线签名**: 用户可以离线创建许可签名，提高安全性
- **ERC-2612 兼容**: 遵循标准的 ERC-2612 permit 接口
- **无需预批准**: 无需事先调用 `approve` 方法

## 方法签名

```solidity
function depositWithPermit(
    bytes32 orderId,
    uint256 amount,
    uint256 deadline,
    uint8 v,
    bytes32 r,
    bytes32 s
) external
```

### 参数说明

- `orderId`: 订单的唯一标识符 (例如，字符串ID的 keccak256 哈希)
- `amount`: 要存入的 USDC 数量
- `deadline`: 许可签名的过期时间戳
- `v`: ECDSA 签名的恢复字节
- `r`: ECDSA 签名对的一半
- `s`: ECDSA 签名对的另一半

## 使用示例

### 1. 创建许可签名 (前端 JavaScript)

```javascript
// 使用 ethers.js v6
async function createPermitSignature(
    signer,
    tokenAddress,
    spenderAddress,
    amount,
    deadline
) {
    // 获取 token 合约
    const token = new ethers.Contract(tokenAddress, ERC20_PERMIT_ABI, signer);
    
    // 获取链ID和 nonce
    const chainId = await signer.getChainId();
    const nonce = await token.nonces(signer.address);
    
    // 定义 EIP-712 域
    const domain = {
        name: await token.name(),
        version: '1',
        chainId: chainId,
        verifyingContract: tokenAddress
    };
    
    // 定义类型
    const types = {
        Permit: [
            { name: 'owner', type: 'address' },
            { name: 'spender', type: 'address' },
            { name: 'value', type: 'uint256' },
            { name: 'nonce', type: 'uint256' },
            { name: 'deadline', type: 'uint256' }
        ]
    };
    
    // 定义值
    const values = {
        owner: signer.address,
        spender: spenderAddress,
        value: amount,
        nonce: nonce,
        deadline: deadline
    };
    
    // 创建签名
    const signature = await signer._signTypedData(domain, types, values);
    const { v, r, s } = ethers.utils.splitSignature(signature);
    
    return { v, r, s };
}
```

### 2. 调用 depositWithPermit

```javascript
async function depositWithPermit(
    paymentVaultContract,
    orderId,
    amount,
    deadline,
    v,
    r,
    s
) {
    const tx = await paymentVaultContract.depositWithPermit(
        orderId,
        amount,
        deadline,
        v,
        r,
        s
    );
    
    const receipt = await tx.wait();
    console.log('Deposit successful:', receipt.transactionHash);
    
    return receipt;
}
```

### 3. 完整的工作流程

```javascript
async function performPermitDeposit() {
    // 配置参数
    const orderId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("order123"));
    const amount = ethers.utils.parseUnits("100", 6); // 100 USDC
    const deadline = Math.floor(Date.now() / 1000) + 3600; // 1小时后过期
    
    // 1. 创建许可签名
    const { v, r, s } = await createPermitSignature(
        signer,
        USDC_TOKEN_ADDRESS,
        PAYMENT_VAULT_ADDRESS,
        amount,
        deadline
    );
    
    // 2. 执行带许可的存款
    const receipt = await depositWithPermit(
        paymentVaultContract,
        orderId,
        amount,
        deadline,
        v,
        r,
        s
    );
    
    console.log('Transaction completed:', receipt);
}
```

## 错误处理

常见的错误和解决方案：

1. **"Permit signature has expired"**: 检查 deadline 是否在未来
2. **"ERC20Permit: invalid signature"**: 验证签名参数是否正确
3. **"Deposit amount must be greater than zero"**: 确保金额大于0
4. **"Token transfer failed"**: 检查用户是否有足够的代币余额

## 燃气费比较

- **传统方式** (两个交易):
  1. `approve()`: ~46,000 gas
  2. `deposit()`: ~55,000 gas
  - **总计**: ~101,000 gas

- **Permit 方式** (一个交易):
  - `depositWithPermit()`: ~77,000 gas
  - **节省**: ~24,000 gas (约 24%)

## 安全注意事项

1. **签名过期**: 始终设置合理的过期时间
2. **重放攻击**: 每个签名只能使用一次 (通过 nonce 防护)
3. **前端安全**: 签名应在客户端创建，不要发送私钥到服务器
4. **合约验证**: 确保与正确的合约地址交互

## 兼容性

- 支持所有实现 ERC-2612 标准的代币
- USDC、USDT、DAI 等主流稳定币都支持 permit 功能
- 在 Ethereum、Polygon、Arbitrum 等网络上都可使用
