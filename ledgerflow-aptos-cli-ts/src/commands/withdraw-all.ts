import { Command } from "commander";
import chalk from "chalk";
import ora from "ora";
import inquirer from "inquirer";
import { createAptosClient } from "../utils/aptos-client";
import { formatAmount } from "../utils/config";

const withdrawAllCommand = new Command("withdraw-all")
  .description("Withdraw all USDC from payment vault")
  .option("-p, --private-key <privateKey>", "Private key of the account")
  .action(async (options) => {
    try {
      let { privateKey } = options;

      // Interactive prompt for missing private key
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

      const client = createAptosClient();
      const spinner = ora("Checking vault balance...").start();

      try {
        const account = await client.getAccountInfo(privateKey);
        const vaultBalance = await client.getVaultBalance(account.address);

        spinner.stop();

        if (vaultBalance === 0) {
          console.log(chalk.yellow("Vault balance is zero. Nothing to withdraw."));
          return;
        }

        console.log(chalk.yellow("\nVault Info:"));
        console.log(chalk.cyan(`Current Balance: ${formatAmount(vaultBalance.toString())} USDC`));
      } catch (error) {
        spinner.fail("Could not fetch vault balance");
        throw error;
      }

      // Confirmation
      const { confirm } = await inquirer.prompt([
        {
          type: "confirm",
          name: "confirm",
          message: "Do you want to withdraw all funds from the vault?",
          default: false,
        },
      ]);

      if (!confirm) {
        console.log(chalk.yellow("Withdrawal cancelled."));
        return;
      }

      const withdrawSpinner = ora("Withdrawing all funds from vault...").start();

      const txHash = await client.withdrawAllFromVault(privateKey);

      withdrawSpinner.succeed(chalk.green(`Withdrawal successful! Transaction hash: ${txHash}`));
      console.log(
        chalk.blue(
          `View on explorer: https://explorer.aptoslabs.com/txn/${txHash}?network=testnet`,
        ),
      );
    } catch (error) {
      console.error(chalk.red("Error withdrawing from vault:"), error);
      process.exit(1);
    }
  });

module.exports = withdrawAllCommand;
