// Script to deposit USDC to LedgerFlow Payment Vault
// This script handles depositing USDC tokens to the vault with an order ID

script {
    use ledgerflow_vault::payment_vault_fa;

    /// Deposit USDC to the payment vault
    /// 
    /// # Parameters
    /// * payer - The signer making the deposit
    /// * vault_address - Address where the vault is deployed
    /// * order_id - Unique identifier for this order (as vector<u8>)
    /// * amount - Amount of USDC to deposit (in micro-USDC, 1 USDC = 1,000,000 micro-USDC)
    fun deposit_usdc(
        payer: &signer,
        vault_address: address,
        order_id: vector<u8>,
        amount: u64
    ) {
        payment_vault_fa::deposit(payer, vault_address, order_id, amount);
    }
}
