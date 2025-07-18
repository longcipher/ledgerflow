use clap::{Parser, Subcommand};
use eyre::Result;

mod commands;
mod contracts;
mod lib_utils;

use commands::{execute_deposit, execute_deposit_with_permit, execute_withdraw};

#[derive(Parser)]
#[command(name = "ledgerflow-eth-cli")]
#[command(about = "A CLI tool for interacting with LedgerFlow PaymentVault contract on Ethereum")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Standard deposit requiring prior USDC approval
    Deposit {
        /// RPC URL for the blockchain network
        #[arg(long, env = "RPC_URL")]
        rpc_url: String,
        /// Private key for the wallet (hex format)
        #[arg(long, env = "PRIVATE_KEY")]
        private_key: String,
        /// PaymentVault contract address
        #[arg(long, env = "VAULT_ADDRESS")]
        contract_address: String,
        /// Order ID (32 bytes hex string)
        #[arg(long)]
        order_id: String,
        /// Amount to deposit (in USDC units, e.g., 1000000 for 1 USDC)
        #[arg(long)]
        amount: u64,
    },
    /// Deposit using ERC-2612 permit signature (gas efficient)
    DepositWithPermit {
        /// RPC URL for the blockchain network
        #[arg(long, env = "RPC_URL")]
        rpc_url: String,
        /// Private key for the wallet (hex format)
        #[arg(long, env = "PRIVATE_KEY")]
        private_key: String,
        /// PaymentVault contract address
        #[arg(long, env = "VAULT_ADDRESS")]
        contract_address: String,
        /// Order ID (32 bytes hex string)
        #[arg(long)]
        order_id: String,
        /// Amount to deposit (in USDC units, e.g., 1000000 for 1 USDC)
        #[arg(long)]
        amount: u64,
        /// Permit deadline timestamp
        #[arg(long)]
        deadline: u64,
    },
    /// Withdraw all USDC from the vault (owner only)
    Withdraw {
        /// RPC URL for the blockchain network
        #[arg(long, env = "RPC_URL")]
        rpc_url: String,
        /// Private key for the wallet (hex format)
        #[arg(long, env = "PRIVATE_KEY")]
        private_key: String,
        /// PaymentVault contract address
        #[arg(long, env = "VAULT_ADDRESS")]
        contract_address: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Deposit {
            rpc_url,
            private_key,
            contract_address,
            order_id,
            amount,
        } => {
            execute_deposit(rpc_url, private_key, contract_address, order_id, amount).await?;
        }
        Commands::DepositWithPermit {
            rpc_url,
            private_key,
            contract_address,
            order_id,
            amount,
            deadline,
        } => {
            execute_deposit_with_permit(
                rpc_url,
                private_key,
                contract_address,
                order_id,
                amount,
                deadline,
            )
            .await?;
        }
        Commands::Withdraw {
            rpc_url,
            private_key,
            contract_address,
        } => {
            execute_withdraw(rpc_url, private_key, contract_address).await?;
        }
    }

    Ok(())
}
