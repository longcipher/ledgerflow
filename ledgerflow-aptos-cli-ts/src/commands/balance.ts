import { Command } from "commander";
import chalk from "chalk";
import ora from "ora";
import inquirer from "inquirer";
import { createAptosClient } from "../utils/aptos-client";
import { formatAmount } from "../utils/config";

const balanceCommand = new Command("balance")
  .description("Check USDC and vault balances")
  .option("-p, --private-key <privateKey>", "Private key of the account")
  .option("-a, --address <address>", "Address to check (alternative to private key)")
  .action(async (options) => {
    try {
      let { privateKey, address } = options;
      let accountAddress: string;

      if (!privateKey && !address) {
        const answer = await inquirer.prompt([
          {
            type: "list",
            name: "method",
            message: "How would you like to check balance?",
            choices: [
              { name: "Private Key", value: "privateKey" },
              { name: "Address", value: "address" },
            ],
          },
        ]);

        if (answer.method === "privateKey") {
          const keyAnswer = await inquirer.prompt([
            {
              type: "password",
              name: "privateKey",
              message: "Enter your private key:",
              mask: "*",
            },
          ]);
          privateKey = keyAnswer.privateKey;
        } else {
          const addressAnswer = await inquirer.prompt([
            {
              type: "input",
              name: "address",
              message: "Enter address to check:",
              validate: (input) => {
                if (!input.startsWith("0x") || input.length !== 66) {
                  return "Invalid address format. Should be 0x followed by 64 characters.";
                }
                return true;
              },
            },
          ]);
          address = addressAnswer.address;
        }
      }

      const client = createAptosClient();
      const spinner = ora("Fetching balances...").start();

      try {
        if (privateKey) {
          const accountInfo = await client.getAccountInfo(privateKey);
          accountAddress = accountInfo.address;
        } else {
          accountAddress = address;
        }

        const [usdcBalance, vaultBalance] = await Promise.all([
          client.getUsdcBalance(accountAddress),
          client.getVaultBalance(accountAddress),
        ]);

        spinner.succeed("Balances fetched successfully!");

        console.log(chalk.yellow(`\nBalances for ${accountAddress}:`));
        console.log(chalk.cyan(`USDC Balance: ${formatAmount(usdcBalance.toString())}`));
        console.log(chalk.cyan(`Vault Balance: ${formatAmount(vaultBalance.toString())}`));
        console.log(
          chalk.cyan(`Total: ${formatAmount((Number(usdcBalance) + vaultBalance).toString())}`),
        );

        console.log(
          chalk.blue(
            `\nView on explorer: https://explorer.aptoslabs.com/account/${accountAddress}?network=testnet`,
          ),
        );
      } catch (error) {
        spinner.fail("Failed to fetch balances");
        throw error;
      }
    } catch (error) {
      console.error(chalk.red("Error checking balances:"), error);
      process.exit(1);
    }
  });

module.exports = balanceCommand;
