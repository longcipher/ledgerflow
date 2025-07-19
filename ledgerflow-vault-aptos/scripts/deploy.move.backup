// Deployment script for LedgerFlow Payment Vault
// This script handles the deployment and initialization of the payment vault on Aptos

script {
    use ledgerflow_vault::payment_vault;

    /// Deploy and initialize the payment vault
    /// This function should be called by the account that will own the vault
    fun deploy_and_initialize(deployer: &signer) {
        // Initialize the payment vault
        payment_vault::initialize(deployer);
    }
}
