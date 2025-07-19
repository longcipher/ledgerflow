// Script to withdraw all USDC from LedgerFlow Payment Vault
// This script handles withdrawing all funds from the vault (owner only)

script {
    use ledgerflow_vault::payment_vault_fa;

    /// Withdraw all USDC from the payment vault
    /// Only the vault owner can call this function
    /// 
    /// # Parameters
    /// * owner - The vault owner's signer
    /// * vault_address - Address where the vault is deployed  
    /// * recipient - Address to receive all withdrawn funds
    fun withdraw_all_usdc(
        owner: &signer,
        vault_address: address,
        recipient: address
    ) {
        payment_vault_fa::withdraw_all(owner, vault_address, recipient);
    }
}
