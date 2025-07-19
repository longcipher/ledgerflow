#!/usr/bin/env node

import { Command } from "commander";
import chalk from "chalk";

const transferUsdcCommand = require("./commands/transfer-usdc");
const depositCommand = require("./commands/deposit");
const withdrawAllCommand = require("./commands/withdraw-all");
const balanceCommand = require("./commands/balance");
const vaultInfoCommand = require("./commands/vault-info");

const program = new Command();

program
  .name("ledgerflow-aptos")
  .description(chalk.blue("🚀 LedgerFlow Aptos CLI - Interact with USDC and PaymentVault"))
  .version("1.0.0");

// Add commands
program.addCommand(transferUsdcCommand);
program.addCommand(depositCommand);
program.addCommand(withdrawAllCommand);
program.addCommand(balanceCommand);
program.addCommand(vaultInfoCommand);

// Global error handler
program.configureOutput({
  writeErr: (str) => process.stderr.write(chalk.red(str)),
});

// Handle unknown commands
program.on("command:*", () => {
  console.error(chalk.red(`Unknown command: ${program.args.join(" ")}`));
  console.log(chalk.yellow("Run --help to see available commands"));
  process.exit(1);
});

// Show help if no command provided
if (!process.argv.slice(2).length) {
  program.outputHelp();
}

program.parse(process.argv);

// Handle promise rejections
process.on("unhandledRejection", (error) => {
  console.error(chalk.red("Unhandled promise rejection:"), error);
  process.exit(1);
});
