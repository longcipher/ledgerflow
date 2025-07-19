import { Command } from "commander";
import chalk from "chalk";
import ora from "ora";
import inquirer from "inquirer";
import { createAptosClient } from "../utils/aptos-client";
import { parseAmount, formatAmount } from "../utils/config";

const depositCommand = new Command("deposit")
  .description("Deposit USDC to payment vault")
  .option("-p, --private-key <privateKey>", "Private key of the account")
  .option("-a, --amount <amount>", "Amount of USDC to deposit (e.g., 1.5)")
  .action(async (options) => {
    try {
      let { privateKey, amount } = options;

      // Interactive prompts for missing values
      if (!privateKey) {
        const answer = await inquirer.prompt([
          {
            type: "password",
            name: "privateKey",
            message: "Enter your private key:",
            mask: "*",
          },
        ]);
        privateKey = answer.privateKey;
      }

      if (!amount) {
        const answer = await inquirer.prompt([
          {
            type: "input",
            name: "amount",
            message: "Enter amount to deposit:",
            validate: (input) => {
              const num = parseFloat(input);
              if (Number.isNaN(num) || num <= 0) {
                return "Please enter a valid positive number.";
              }
              return true;
            },
          },
        ]);
        amount = answer.amount;
      }

      const amountValue = parseAmount(amount);

      // Show current balance
      const client = createAptosClient();
      const spinner = ora("Checking current balance...").start();

      try {
        const account = await client.getAccountInfo(privateKey);
        const usdcBalance = await client.getUsdcBalance(account.address);
        const vaultBalance = await client.getVaultBalance(account.address);

        spinner.stop();

        console.log(chalk.yellow("\nCurrent Balances:"));
        console.log(chalk.cyan(`USDC: ${formatAmount(usdcBalance)}`));
        console.log(chalk.cyan(`Vault: ${formatAmount(vaultBalance.toString())}`));
      } catch (error) {
        spinner.stop();
        console.log(chalk.yellow("Could not fetch current balances"));
      }

      // Confirmation
      console.log(chalk.yellow("\nDeposit Summary:"));
      console.log(chalk.cyan(`Amount: ${formatAmount(amountValue)} USDC`));

      const { confirm } = await inquirer.prompt([
        {
          type: "confirm",
          name: "confirm",
          message: "Do you want to proceed with this deposit?",
          default: false,
        },
      ]);

      if (!confirm) {
        console.log(chalk.yellow("Deposit cancelled."));
        return;
      }

      const depositSpinner = ora("Depositing to vault...").start();

      const txHash = await client.depositToVault(privateKey, Number(amountValue));

      depositSpinner.succeed(chalk.green(`Deposit successful! Transaction hash: ${txHash}`));
      console.log(
        chalk.blue(
          `View on explorer: https://explorer.aptoslabs.com/txn/${txHash}?network=testnet`,
        ),
      );
    } catch (error) {
      console.error(chalk.red("Error depositing to vault:"), error);
      process.exit(1);
    }
  });

module.exports = depositCommand;
