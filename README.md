# LedgerFlow

**Making value flow as freely, efficiently, and borderlessly as information flow.**

LedgerFlow is a modern payment gateway built on blockchain technology, centered around stablecoins (such as USDC). We aim to solve the pain points of current mainstream payment systems (such as Stripe), including high registration barriers, opaque fees, account freezing risks, and difficult appeals.

By leveraging blockchain's transparency, security, and composability, LedgerFlow provides SaaS service providers, developers, and independent creators worldwide with a **low-barrier, low-cost, high-efficiency, censorship-resistant** payment solution.

## Testnet(Unichain Sepolia) Demo

* PaymentVault Contract: [0x8b6f22009ae835795b9b33d75ad218c730db039b](https://sepolia.uniscan.xyz/address/0x8b6f22009ae835795b9b33d75ad218c730db039b)

## 🎯 Problems We Solve

Current centralized payment gateways, while powerful, have inherent flaws that create barriers for emerging digital economies and global collaboration:

- **High Entry Barriers**: Must register a company entity to open an account, excluding many independent developers and small teams
- **Account Freezing Risk**: Platforms have unilateral power to freeze accounts, lack of fund security guarantees, lengthy and low-success appeal processes
- **Complexity & Learning Costs**: Integrating traditional payment systems requires significant learning costs and development resources
- **Geographic & Banking Restrictions**: Business scope limited to supported countries and banking systems, cannot achieve true global coverage
- **Opaque Fees**: Hidden currency conversion fees, cross-border transaction fees make final costs unpredictable

## 🚀 Our Solution

LedgerFlow fundamentally solves these problems through the Web3 technology stack:

### Core Advantages

- **Permissionless**: Anyone with an EVM address can start receiving payments immediately, no company registration required
- **Self-Custody**: Funds go directly into on-chain Vault contracts you control, platform cannot freeze or misappropriate funds
- **Truly Global**: Can receive payments anywhere in the world with internet connection, no geographic restrictions
- **Transparent Fees**: Fees only include predictable blockchain network Gas fees, no hidden charges
- **Simple & Decoupled**: Easy integration through simple APIs, business systems completely decoupled from on-chain funds

## 🏗️ System Architecture

LedgerFlow uses a lightweight decoupled architecture consisting of the following core components:

```text
ledgerflow/                    # Project root directory
├── ledgerflow-vault/               # Smart contract module (PaymentVault)
│   ├── src/                        # Contract source code
│   ├── test/                       # Contract tests
│   ├── script/                     # Deployment scripts
│   └── ...                         # Contract-related files
├── ledgerflow-balancer/            # Backend service (business logic core)
├── ledgerflow-bot/                 # Telegram Bot (user frontend)
├── ledgerflow-cli/                 # Command-line tool
├── ledgerflow-indexer/             # Event indexer (on-chain monitoring)
└── ...                             # Workspace configuration
```

### Component Description

1. **PaymentVault Contract (Smart Contract)**
   - Serves as the sole entry point and vault for funds, receiving and storing all USDC payments
   - Supports both standard `approve/deposit` and `permit/deposit` modes
   - Triggers events for Indexer monitoring, enabling on-chain and off-chain data synchronization

2. **Indexer (Event Indexer)**
   - Real-time monitoring of DepositReceived events from PaymentVault contracts across multiple chains
   - Parses event data and updates order status to "completed"

3. **Balancer (Backend Service)**
   - Business logic core of the system, providing REST APIs
   - Handles account management, order creation, status queries, balance calculations, and other business functions
   - Connects user frontend with off-chain data

4. **Telegram Bot (User Frontend)**
   - Primary interface for users to interact with the LedgerFlow system
   - Handles user onboarding, payment initiation, status notifications, balance queries, and other functions

## 🔄 Payment Flow

1. **Merchant initiates payment request**: Input "I want to receive 10 USDC" through Telegram Bot
2. **System generates order**: Bot calls Balancer API to generate unique `orderId`
3. **Display payment details**: Bot shows payer the payment address, amount, and order information
4. **On-chain payment**: Payer uses wallet to send USDC to PaymentVault contract
5. **Event monitoring**: Indexer captures DepositReceived event
6. **Status update**: Indexer updates order status to "completed" in database
7. **Confirmation notification**: Merchant receives payment success notification

## 🌟 Core Features

### 1. Non-Custodial Vault

- Uses a single PaymentVault smart contract as the fund aggregation entry point
- Eliminates complexity and security risks of server private key management
- Supports secure storage of large amounts of funds

### 2. Seamless Multi-Chain Support

- Can be deployed on any EVM-compatible chain
- Supports Ethereum, Polygon, Arbitrum, Optimism, Base, BNB Chain, etc.
- Merchants can freely choose to enable payment collection on one or multiple chains based on needs

### 3. Programmable & Composable

- **Subscription payments**: Support time-locked automatic deduction subscription models
- **DeFi integration**: Idle funds can be combined with Staking, Lending and other protocols to generate additional yield
- **Multi-currency support**: Can be combined with DEX aggregators to support payments in any token

### 4. User-Friendly Payment Experience

- Supports EIP-2612 permit signatures for "user-side gasless" experience
- One off-chain signature completes authorization and payment
- Greatly improves conversion rates and user experience

## 🎯 Quick Start

For detailed usage instructions, please refer to the README.md files in each module:

- [Smart Contract Deployment and Usage](./ledgerflow-vault/README.md)
- [Backend Service Configuration](./ledgerflow-balancer/README.md)
- [Telegram Bot Setup](./ledgerflow-bot/README.md)
- [Event Indexer Configuration](./ledgerflow-indexer/README.md)
- [Command Line Tool Usage](./ledgerflow-cli/README.md)

## 🔮 Future Vision

- **SaaS Merchant Dashboard**: Develop Web frontend for merchants to manage orders and data more intuitively
- **One-Click Plugin Integration**: Develop payment plugins for mainstream e-commerce platforms
- **Subscription & Recurring Payments**: Implement authorization and time-lock logic at the contract level
- **Business Model**: Free usage initially, future minimal service fees (0.1% - 0.25%) on withdrawals

---

**Let's build a more open, transparent, and efficient global payment network together!**
