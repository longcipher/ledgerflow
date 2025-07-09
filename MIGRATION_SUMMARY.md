# LedgerFlow 统一数据库迁移系统

## 项目概述

已成功创建了一个统一的数据库迁移管理系统 `ledgerflow-migrations`，用于管理整个 LedgerFlow 项目中所有子项目的数据库迁移，避免冲突并确保数据库schema的一致性。

## 完成的工作

### 1. 创建了新的 migrations crate

- **位置**: `/Users/akagi201/src/github.com/longcipher/ledgerflow/ledgerflow-migrations/`
- **功能**: 统一管理所有数据库迁移
- **依赖**: 使用 sqlx 和 sqlx-cli 进行迁移管理

### 2. 整合了所有子项目的迁移

从以下子项目整合了数据库schema：
- **ledgerflow-balancer**: accounts 和 orders 表
- **ledgerflow-bot**: users 表和相关索引
- **ledgerflow-indexer**: chain_states 和 deposit_events 表

### 3. 统一的数据库schema

创建了单一的迁移文件 `20250709000001_initial_schema.sql`，包含：

#### 表结构
- `accounts` - 用户账户信息 (来自 balancer)
- `users` - Telegram 用户信息 (来自 bot)
- `orders` - 订单管理 (统一所有服务)
- `chain_states` - 区块链扫描状态 (来自 indexer)
- `deposit_events` - 存款事件日志 (来自 indexer)

#### 其他结构
- `order_status` ENUM 类型
- 所有表的性能优化索引
- 自动 `updated_at` 时间戳触发器

### 4. 工具和脚本

- **migrate.sh**: 迁移操作脚本
- **Makefile**: 常用任务的便捷命令
- **config.yaml**: 配置文件管理
- **Dockerfile**: 容器化支持

### 5. 代码结构

```
ledgerflow-migrations/
├── src/
│   ├── lib.rs          # 核心 MigrationManager 库
│   ├── main.rs         # CLI 二进制文件
│   └── tests.rs        # 单元测试
├── migrations/
│   └── 20250709000001_initial_schema.sql  # 统一schema
├── config.yaml         # 配置文件
├── migrate.sh          # 迁移脚本
├── Makefile           # 构建命令
├── Dockerfile         # 容器支持
├── README.md          # 文档
├── INTEGRATION.md     # 集成指南
└── PROJECT_STATUS.md  # 项目状态
```

## 使用方法

### 基本操作

```bash
# 进入 migrations 目录
cd ledgerflow-migrations

# 安装 sqlx-cli（如果尚未安装）
make install

# 设置数据库并运行迁移
make setup

# 运行待执行的迁移
make migrate

# 添加新的迁移
make add NAME="migration_name"

# 查看迁移状态
make info

# 重置数据库
make reset
```

### 在其他服务中使用

在每个子项目的 `Cargo.toml` 中添加：

```toml
[dependencies]
ledgerflow-migrations = { path = "../ledgerflow-migrations" }
```

在服务启动代码中：

```rust
use ledgerflow_migrations::MigrationManager;

async fn setup_database() -> Result<sqlx::PgPool, Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let migration_manager = MigrationManager::new(Some(&database_url)).await?;
    migration_manager.run_migrations().await?;
    Ok(migration_manager.get_pool().clone())
}
```

## 核心特性

### 1. 统一管理
- 所有迁移文件集中管理
- 避免各服务间的schema冲突
- 确保整个系统的数据一致性

### 2. 配置管理
- 支持环境特定配置
- 数据库连接池参数可配置
- 支持环境变量覆盖

### 3. 工具支持
- 丰富的命令行工具
- Docker 容器化支持
- CI/CD 集成示例

### 4. 测试覆盖
- 单元测试验证核心功能
- 配置加载测试
- 迁移目录结构验证

## 下一步工作

### 集成到各个服务

1. **更新 ledgerflow-balancer**
   - 移除 `migrations/` 目录
   - 更新 `src/database.rs` 使用统一迁移
   - 在 `Cargo.toml` 中添加依赖

2. **更新 ledgerflow-bot**
   - 移除 `migrations/` 目录
   - 更新 `src/database.rs` 使用统一迁移
   - 在 `Cargo.toml` 中添加依赖

3. **更新 ledgerflow-indexer**
   - 移除 `migrations/` 目录
   - 更新 `src/database.rs` 使用统一迁移
   - 在 `Cargo.toml` 中添加依赖

### 增强功能

- [ ] 实现迁移回滚机制
- [ ] 添加迁移验证
- [ ] 创建迁移状态报告
- [ ] 添加性能监控

## 测试验证

系统已通过以下测试：

```bash
$ cargo test
running 3 tests
test tests::tests::test_migration_directory_exists ... ok
test tests::tests::test_config_loading ... ok
test tests::tests::test_migration_manager_creation ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 文档

- **README.md**: 使用说明和API文档
- **INTEGRATION.md**: 详细集成指南
- **PROJECT_STATUS.md**: 项目状态和路线图

## 环境要求

- Rust 1.75+
- PostgreSQL 12+
- sqlx-cli (`cargo install sqlx-cli --no-default-features --features postgres`)

## 配置示例

```yaml
database:
  url: "postgresql://postgres:password@localhost/ledgerflow"
  max_connections: 5
  min_connections: 1

migrations:
  path: "./migrations"
  table: "_sqlx_migrations"
```

这个统一的迁移系统为整个 LedgerFlow 项目提供了一个清晰、一致且可维护的数据库schema管理解决方案。
