import { Command } from "commander";
import chalk from "chalk";
import ora from "ora";
import inquirer from "inquirer";
import { createAptosClient } from "../utils/aptos-client";

const vaultInfoCommand = new Command("vault-info")
  .description("Get detailed vault information")
  .option("-a, --address <address>", "Vault address to check")
  .action(async (options) => {
    try {
      let { address } = options;

      if (!address) {
        const answer = await inquirer.prompt([
          {
            type: "input",
            name: "address",
            message: "Enter vault address:",
            validate: (input) => {
              if (!input.startsWith("0x") || input.length !== 66) {
                return "Invalid address format. Should be 0x followed by 64 characters.";
              }
              return true;
            },
          },
        ]);
        address = answer.address;
      }

      const client = createAptosClient();
      const spinner = ora("Fetching vault information...").start();

      try {
        const vaultInfo = await client.getVaultInfo(address);

        spinner.succeed("Vault information fetched successfully!");

        console.log(chalk.yellow("\nVault Information:"));
        console.log(chalk.cyan(`Address: ${address}`));
        console.log(chalk.cyan(`Exists: ${vaultInfo.exists}`));
        console.log(chalk.cyan(`Owner: ${vaultInfo.owner}`));
        console.log(chalk.cyan(`Balance: ${vaultInfo.balance}`));
        console.log(chalk.cyan(`Deposit Count: ${vaultInfo.depositCount}`));
        console.log(chalk.cyan(`Created At: ${vaultInfo.createdAt}`));
        console.log(chalk.cyan(`USDC Metadata Address: ${vaultInfo.usdcMetadataAddress}`));

        console.log(
          chalk.blue(
            `\nView on explorer: https://explorer.aptoslabs.com/account/${address}?network=testnet`,
          ),
        );
      } catch (error) {
        spinner.fail("Failed to fetch vault information");
        throw error;
      }
    } catch (error) {
      console.error(chalk.red("Error fetching vault info:"), error);
      process.exit(1);
    }
  });

module.exports = vaultInfoCommand;
