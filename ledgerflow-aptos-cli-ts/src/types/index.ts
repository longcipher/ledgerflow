import type { Network } from "@aptos-labs/ts-sdk";

export interface Config {
  network: Network;
  privateKey?: string;
  vaultAddress: string;
  usdcMetadataAddress: string;
}

export interface VaultInfo {
  exists: boolean;
  owner: string;
  balance: string;
  depositCount: string;
  createdAt: string;
  usdcMetadataAddress: string;
}

export interface TransferOptions {
  privateKey: string;
  recipient: string;
  amount: string;
  network?: Network;
}

export interface DepositOptions {
  privateKey: string;
  vaultAddress: string;
  orderId: string;
  amount: string;
  network?: Network;
}

export interface WithdrawOptions {
  privateKey: string;
  vaultAddress: string;
  recipient: string;
  network?: Network;
}

export interface BalanceOptions {
  address: string;
  network?: Network;
}

export interface VaultInfoOptions {
  vaultAddress: string;
  network?: Network;
}
