// Deployment script for LedgerFlow Payment Vault (Fungible Asset version)
// This script handles the deployment and initialization of the payment vault on Aptos
// using Fungible Assets for USDC

script {
    use ledgerflow_vault::payment_vault_fa;

    /// Deploy and initialize the payment vault with USDC Fungible Asset
    /// This function should be called by the account that will own the vault
    /// 
    /// # Parameters
    /// * deployer - The signer who will become the vault owner
    /// * usdc_metadata_addr - The address of the USDC metadata object (0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832)
    fun deploy_and_initialize_fa(deployer: &signer, usdc_metadata_addr: address) {
        // Initialize the payment vault with USDC FA
        payment_vault_fa::initialize(deployer, usdc_metadata_addr);
    }
}
