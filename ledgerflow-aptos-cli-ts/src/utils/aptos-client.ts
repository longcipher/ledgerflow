import {
  Aptos,
  AptosConfig,
  Network,
  Account,
  Ed25519PrivateKey,
  AccountAddress,
} from "@aptos-labs/ts-sdk";
import type { Config } from "../types";

export class AptosClient {
  private aptos: Aptos;
  public config: Config;

  constructor(config: Config) {
    this.config = config;
    const aptosConfig = new AptosConfig({ network: config.network });
    this.aptos = new Aptos(aptosConfig);
  }

  /**
   * Create account from private key
   */
  private createAccountFromPrivateKey(privateKeyHex: string): Account {
    return Account.fromPrivateKey({
      privateKey: new Ed25519PrivateKey(privateKeyHex),
    });
  }

  /**
   * Get account info from private key
   */
  async getAccountInfo(privateKeyHex: string) {
    const account = this.createAccountFromPrivateKey(privateKeyHex);
    return {
      address: account.accountAddress.toString(),
      account: account,
    };
  }

  /**
   * Get vault balance for an account
   */
  async getVaultBalance(address: string): Promise<number> {
    try {
      const result = await this.aptos.view({
        payload: {
          function: `${this.config.vaultAddress}::payment_vault_fa::get_balance`,
          functionArguments: [AccountAddress.fromString(address)],
        },
      });
      return Number(result[0]);
    } catch {
      return 0; // Return 0 if vault doesn't exist or has no balance
    }
  }

  static createAccount(privateKeyHex: string): Account {
    const privateKey = new Ed25519PrivateKey(privateKeyHex);
    return Account.fromPrivateKey({ privateKey });
  }

  async getAccountBalance(address: string): Promise<string> {
    try {
      const balance = await this.aptos.getAccountAPTAmount({
        accountAddress: address,
      });
      return balance.toString();
    } catch (error) {
      throw new Error(`Failed to get APT balance: ${error}`);
    }
  }

  async getUsdcBalance(address: string): Promise<string> {
    try {
      const resource = await this.aptos.getAccountResource({
        accountAddress: address,
        resourceType: `0x1::primary_fungible_store::PrimaryFungibleStore`,
      });

      // This is a simplified approach - in reality, you'd need to query the specific USDC store
      // For now, return "0" as a placeholder
      return "0";
    } catch (error) {
      return "0";
    }
  }

  async transferUsdc(
    privateKeyHex: string,
    recipientAddress: string,
    amount: number,
  ): Promise<string> {
    try {
      const account = this.createAccountFromPrivateKey(privateKeyHex);

      const transaction = await this.aptos.transferCoinTransaction({
        sender: account.accountAddress,
        recipient: recipientAddress,
        amount: BigInt(amount),
        coinType: `${this.config.usdcMetadataAddress}::coin::T`,
      });

      const committedTxn = await this.aptos.signAndSubmitTransaction({
        signer: account,
        transaction,
      });

      const executedTransaction = await this.aptos.waitForTransaction({
        transactionHash: committedTxn.hash,
      });

      return executedTransaction.hash;
    } catch (error) {
      throw new Error(`Failed to transfer USDC: ${error}`);
    }
  }

  async depositToVault(privateKeyHex: string, amount: number): Promise<string> {
    try {
      const account = this.createAccountFromPrivateKey(privateKeyHex);

      const transaction = await this.aptos.transaction.build.simple({
        sender: account.accountAddress,
        data: {
          function: `${this.config.vaultAddress}::payment_vault_fa::deposit`,
          functionArguments: [this.config.vaultAddress, amount.toString()],
        },
      });

      const committedTxn = await this.aptos.signAndSubmitTransaction({
        signer: account,
        transaction,
      });

      const executedTransaction = await this.aptos.waitForTransaction({
        transactionHash: committedTxn.hash,
      });

      return executedTransaction.hash;
    } catch (error) {
      throw new Error(`Failed to deposit to vault: ${error}`);
    }
  }

  async withdrawAllFromVault(privateKeyHex: string): Promise<string> {
    try {
      const account = this.createAccountFromPrivateKey(privateKeyHex);

      const transaction = await this.aptos.transaction.build.simple({
        sender: account.accountAddress,
        data: {
          function: `${this.config.vaultAddress}::payment_vault_fa::withdraw_all`,
          functionArguments: [this.config.vaultAddress, account.accountAddress],
        },
      });

      const committedTxn = await this.aptos.signAndSubmitTransaction({
        signer: account,
        transaction,
      });

      const executedTransaction = await this.aptos.waitForTransaction({
        transactionHash: committedTxn.hash,
      });

      return executedTransaction.hash;
    } catch (error) {
      throw new Error(`Failed to withdraw from vault: ${error}`);
    }
  }

  async getVaultInfo(vaultAddress: string) {
    try {
      const [exists, owner, balance, depositCount, createdAt, usdcMetadataAddress] =
        await Promise.all([
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::vault_exists`,
              functionArguments: [vaultAddress],
            },
          }),
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::get_owner`,
              functionArguments: [vaultAddress],
            },
          }),
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::get_balance`,
              functionArguments: [vaultAddress],
            },
          }),
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::get_deposit_count`,
              functionArguments: [vaultAddress],
            },
          }),
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::get_created_at`,
              functionArguments: [vaultAddress],
            },
          }),
          this.aptos.view({
            payload: {
              function: `${vaultAddress}::payment_vault_fa::get_usdc_metadata_address`,
              functionArguments: [vaultAddress],
            },
          }),
        ]);

      return {
        exists: exists[0] as boolean,
        owner: owner[0] as string,
        balance: balance[0] as string,
        depositCount: depositCount[0] as string,
        createdAt: createdAt[0] as string,
        usdcMetadataAddress: usdcMetadataAddress[0] as string,
      };
    } catch (error) {
      throw new Error(`Failed to get vault info: ${error}`);
    }
  }

  getExplorerUrl(txHash: string): string {
    const baseUrl =
      this.config.network === Network.MAINNET
        ? "https://explorer.aptoslabs.com"
        : "https://explorer.aptoslabs.com";
    const networkParam = this.config.network === Network.MAINNET ? "" : "?network=testnet";
    return `${baseUrl}/txn/${txHash}${networkParam}`;
  }
}

/**
 * Create and return configured Aptos client
 */
export function createAptosClient(customConfig?: Partial<Config>): AptosClient {
  const config: Config = {
    network: Network.TESTNET,
    vaultAddress: "0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846",
    usdcMetadataAddress: "0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832",
    ...customConfig,
  };

  return new AptosClient(config);
}
