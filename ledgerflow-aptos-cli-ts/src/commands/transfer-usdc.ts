import { Command } from "commander";
import chalk from "chalk";
import ora from "ora";
import inquirer from "inquirer";
import { createAptosClient } from "../utils/aptos-client";
import { parseAmount, formatAmount } from "../utils/config";

const transferUsdcCommand = new Command("transfer-usdc")
  .description("Transfer USDC to another address")
  .option("-p, --private-key <privateKey>", "Private key of the sender account")
  .option("-t, --to <address>", "Recipient address")
  .option("-a, --amount <amount>", "Amount of USDC to transfer (e.g., 1.5)")
  .action(async (options) => {
    try {
      let { privateKey, to, amount } = options;

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

      if (!to) {
        const answer = await inquirer.prompt([
          {
            type: "input",
            name: "to",
            message: "Enter recipient address:",
            validate: (input) => {
              if (!input.startsWith("0x") || input.length !== 66) {
                return "Invalid address format. Should be 0x followed by 64 characters.";
              }
              return true;
            },
          },
        ]);
        to = answer.to;
      }

      if (!amount) {
        const answer = await inquirer.prompt([
          {
            type: "input",
            name: "amount",
            message: "Enter amount to transfer:",
            validate: (input) => {
              const num = parseFloat(input);
              if (isNaN(num) || num <= 0) {
                return "Please enter a valid positive number.";
              }
              return true;
            },
          },
        ]);
        amount = answer.amount;
      }

      const amountValue = parseAmount(amount);

      // Confirmation
      console.log(chalk.yellow("\nTransaction Summary:"));
      console.log(chalk.cyan(`From: ${privateKey.slice(0, 10)}...`));
      console.log(chalk.cyan(`To: ${to}`));
      console.log(chalk.cyan(`Amount: ${formatAmount(amountValue)} USDC`));

      const { confirm } = await inquirer.prompt([
        {
          type: "confirm",
          name: "confirm",
          message: "Do you want to proceed with this transfer?",
          default: false,
        },
      ]);

      if (!confirm) {
        console.log(chalk.yellow("Transfer cancelled."));
        return;
      }

      const spinner = ora("Sending USDC transfer transaction...").start();

      const client = createAptosClient();
      const txHash = await client.transferUsdc(privateKey, to, Number(amountValue));

      spinner.succeed(chalk.green(`USDC transfer successful! Transaction hash: ${txHash}`));
      console.log(
        chalk.blue(
          `View on explorer: https://explorer.aptoslabs.com/txn/${txHash}?network=testnet`,
        ),
      );
    } catch (error) {
      console.error(chalk.red("Error transferring USDC:"), error);
      process.exit(1);
    }
  });

module.exports = transferUsdcCommand;
