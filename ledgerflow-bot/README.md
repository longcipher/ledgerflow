# LedgerFlow Bot

LedgerFlow Bot is a Telegram bot that serves as the primary user interface for the LedgerFlow payment system. It allows users to create payment requests, manage their wallets, and track their payment history through a conversational interface.

## Features

- **Session-based registration**: Users register via a conversational flow (email → username → wallet auto-generated)
- **Menu-driven UX**: All actions (deposit, withdraw, account info) are accessible via inline keyboard menus
- **Fully-custodial wallet**: Each user gets a unique, encrypted wallet managed by the system
- **Admin-only withdraw**: Only admin users can trigger withdrawals
- **Deposit flow**: Users can deposit by entering an amount, receiving an order, and sending funds
- **Order notifications**: Users are notified when their deposit is confirmed
- **Stateful user sessions**: The bot remembers where each user is in the flow
- **English-only interface**: All prompts, errors, and menus are in English

## Technology Stack

- **Rust**: Core language for performance and safety
- **Teloxide**: Telegram Bot API framework
- **SQLx**: Database ORM with compile-time SQL checking
- **Alloy**: Ethereum/EVM blockchain interaction
- **PostgreSQL**: Database for user data and order tracking
- **Reqwest**: HTTP client for API communication
- **Tracing**: Structured logging and observability

## Architecture

### User Flow

1. **Registration**: User starts with `/start` or the bot's start button
   - Bot asks for email
   - Bot asks for username
   - Bot creates a custodial wallet and account
   - Bot shows account info and main menu
2. **Main Menu**: User can choose:
   - Deposit (enter amount, get order, send funds)
   - Withdraw (admin only)
   - View account info
   - Return to main menu
3. **Deposit**:
   - User enters amount
   - Bot creates order and shows deposit address/order ID
   - User sends funds
   - Bot notifies user when deposit is confirmed
4. **Withdraw**:
   - Only available to admin users
   - Admin can trigger withdrawal for a user
5. **Notifications**:
   - Bot periodically checks for completed orders and notifies users

All flows are stateful and menu-driven, with clear English prompts and error messages. Legacy commands and Chinese prompts have been removed.
