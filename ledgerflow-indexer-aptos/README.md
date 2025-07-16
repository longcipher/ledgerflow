# LedgerFlow Aptos Indexer

This service indexes `DepositReceived` events from the Aptos PaymentVault contract and stores them in a PostgreSQL database.

## Features
- Listens to Aptos chain events using `aptos-indexer-processor-sdk`
- Filters and stores deposit events for off-chain business logic
- Uses Diesel ORM (required by SDK)

## Usage

1. **Configure**
   - Copy `config.yaml.example` to `config.yaml` and fill in your contract address and database credentials.

2. **Run Migrations**
   ```sh
   diesel migration run --database-url=postgresql://postgres:password@localhost:5432/ledgerflow
   ```

3. **Run the Indexer**
   ```sh
   cargo run --release -- -c config.yaml
   ```

## Database Table
- `aptos_deposit_events`: Stores all deposit events with unique constraint on (chain_id, contract_address, txn_version, event_index)

## Event Structure
- payer: address
- order_id: vector<u8> (stored as hex string)
- amount: u64 (stored as numeric string)
- timestamp: u64 (stored as timestamptz)
- deposit_index: u64
- txn_version: u64
- event_index: u32

---

See the main project README for architecture and integration details.
