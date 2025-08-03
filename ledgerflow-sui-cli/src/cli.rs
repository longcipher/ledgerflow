use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// LedgerFlow Sui CLI - Command-line interface for interacting with LedgerFlow payment vault contracts
#[derive(Parser)]
#[command(
    name = "ledgerflow-sui-cli",
    about = "Command-line interface for LedgerFlow Sui payment vault contracts",
    version = env!("CARGO_PKG_VERSION"),
    author = "LedgerFlow Team"
)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format
    #[arg(long, global = true, default_value = "pretty")]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize configuration file
    Init {
        /// Path to create configuration file
        #[arg(short, long, default_value = "config.yaml")]
        path: PathBuf,
        /// Overwrite existing configuration file
        #[arg(short, long)]
        force: bool,
    },
    /// Deposit USDC to the payment vault
    Deposit {
        /// Unique order identifier (will be converted to bytes)
        #[arg(short, long)]
        order_id: String,
        /// Amount to deposit (in USDC smallest units, typically 6 decimals)
        #[arg(short, long)]
        amount: u64,
        /// Dry run - simulate transaction without submitting
        #[arg(long)]
        dry_run: bool,
    },
    /// Withdraw USDC from the payment vault
    Withdraw {
        /// Recipient address to receive the withdrawn funds
        #[arg(short, long)]
        recipient: String,
        /// Amount to withdraw (in USDC smallest units)
        #[arg(short, long)]
        amount: u64,
        /// Withdraw all available funds
        #[arg(long)]
        all: bool,
        /// Dry run - simulate transaction without submitting
        #[arg(long)]
        dry_run: bool,
    },
    /// Get vault information and balances
    Info {
        /// Show account balance as well
        #[arg(long)]
        include_account: bool,
    },
    /// Get account information
    Account {
        /// Show private account information (private key, etc.)
        #[arg(long)]
        show_private: bool,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output with colors and formatting
    Pretty,
    /// JSON output for programmatic use
    Json,
    /// Compact text output
    Compact,
}
