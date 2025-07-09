# LedgerFlow Balancer - 日志增强说明

## 概述
已为 `ledgerflow-balancer` 添加了详细的日志记录功能，以便更好地跟踪程序运行阶段，而不修改任何业务逻辑。

## 新增日志功能

### 1. 启动阶段日志
- 🚀 **程序启动**: 显示服务启动信息
- 📋 **配置加载**: 显示配置文件加载状态
- 🔗 **数据库连接**: 显示数据库连接状态
- 🔄 **后台任务**: 显示后台任务启动状态
- 🏗️ **路由构建**: 显示应用路由构建状态
- 🌐 **服务绑定**: 显示服务器绑定地址
- 🎯 **服务就绪**: 显示服务就绪状态
- 💡 **端点列表**: 显示所有可用的API端点

### 2. 请求处理日志
- 📝 **API请求**: 记录各种API请求的处理
- 🏥 **健康检查**: 记录健康检查请求
- 👤 **账户注册**: 记录账户注册过程
- 📦 **订单创建**: 记录订单创建过程
- 💰 **余额查询**: 记录余额查询请求

### 3. 后台任务日志
- 🔄 **任务循环**: 显示后台任务循环状态
- ⏸️ **空闲状态**: 显示没有订单需要处理时的状态
- ✅ **成功处理**: 显示订单处理成功信息
- ❌ **处理失败**: 显示订单处理失败信息
- 📊 **批处理统计**: 显示批处理完成统计

## 日志级别设置

### 环境变量
```bash
export RUST_LOG=info
```

### 日志级别说明
- `error`: 只显示错误信息
- `warn`: 显示警告和错误
- `info`: 显示信息、警告和错误 (推荐)
- `debug`: 显示调试信息
- `trace`: 显示所有日志信息

## 使用方法

### 1. 直接运行
```bash
RUST_LOG=info cargo run --bin ledgerflow-balancer
```

### 2. 使用演示脚本
```bash
./demo_logs.sh
```

## 日志示例

### 启动时的日志输出
```
🚀 LedgerFlow Balancer starting up...
📋 Loading configuration from config.yaml
✅ Configuration loaded successfully from config.yaml
🔗 Connecting to database...
✅ Database connected successfully
🔄 Starting background task for processing deposited orders...
✅ Background task started successfully
🏗️ Building application routes...
🌐 Binding server to 0.0.0.0:8080
🎯 LedgerFlow Balancer is ready and listening on 0.0.0.0:8080
💡 Available endpoints:
   - GET  /health - Health check
   - POST /register - Register new account
   - GET  /accounts/username/{username} - Get account by username
   - GET  /accounts/email/{email} - Get account by email
   - GET  /accounts/telegram/{telegram_id} - Get account by telegram ID
   - POST /orders - Create new order
   - GET  /orders/{order_id} - Get order by ID
   - GET  /accounts/{account_id}/balance - Get account balance
   - GET  /admin/orders - List pending orders
```

### 运行时的日志输出
```
🏥 Health check requested
Creating order for account 1: amount=100.0, token=0x123..., chain_id=1
Generated order ID: ledgerflow-1-1234567890 for account 1
Order created successfully: ledgerflow-1-1234567890
🔄 Background task: Starting deposited orders processing loop
Processing 2 deposited orders
✅ Successfully processed deposited order: order-123, amount: 50.0 for account 1
✅ Successfully processed deposited order: order-124, amount: 25.0 for account 2
✅ Batch processing completed: 2/2 orders processed successfully
```

## 优点

1. **无侵入性**: 不修改任何业务逻辑
2. **阶段清晰**: 清晰显示程序运行的各个阶段
3. **问题诊断**: 便于定位问题和调试
4. **监控友好**: 方便运维监控和日志分析
5. **用户友好**: 使用表情符号和清晰的文本描述

## 注意事项

1. 日志级别设置为 `info` 时不会显示调试信息
2. 生产环境建议使用 `warn` 或 `error` 级别以减少日志量
3. 日志文件可以通过重定向保存到文件中
4. 可以配合日志收集系统（如ELK Stack）进行集中管理

## 相关文件

- `src/main.rs` - 主程序启动日志
- `src/services.rs` - 业务服务日志
- `src/handlers.rs` - API请求处理日志
- `demo_logs.sh` - 日志演示脚本
