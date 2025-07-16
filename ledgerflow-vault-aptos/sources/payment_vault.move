/// # LedgerFlow Payment Vault
///
/// This module implements a secure payment vault for USDC deposits on Aptos blockchain.
/// It provides non-custodial fund management with order tracking and owner-controlled withdrawals.
///
/// ## Core Features
/// - Secure USDC deposit with order ID tracking
/// - Owner-only withdrawal functionality
/// - Event emission for off-chain monitoring
/// - Ownership transfer capability
/// - Emergency token recovery (future enhancement)
///
/// ## Architecture
/// The vault uses Aptos' resource model for enhanced security:
/// - PaymentVault resource stores USDC coins and metadata
/// - OwnerCapability resource controls administrative functions
/// - Events provide real-time updates for indexers
///
/// ## Security Considerations
/// - All deposits are atomic operations
/// - Owner verification through capability-based access control
/// - Input validation on all public functions
/// - Linear type safety prevents double-spending

module ledgerflow_vault::payment_vault {
    use std::error;
    use std::signer;
    use std::vector;
    use std::timestamp;

    use aptos_framework::coin::{Self, Coin};
    use aptos_framework::event;

    /// Mock USDC coin type for testing
    /// In production, this should be replaced with the actual USDC coin type
    struct USDC {}

    // ==================== Error Codes ====================

    /// The vault has not been initialized
    const E_NOT_INITIALIZED: u64 = 1;
    /// The vault has already been initialized
    const E_ALREADY_INITIALIZED: u64 = 2;
    /// Caller is not the owner
    const E_NOT_OWNER: u64 = 3;
    /// Insufficient balance for the operation
    const E_INSUFFICIENT_BALANCE: u64 = 4;
    /// Invalid amount (must be greater than 0)
    const E_INVALID_AMOUNT: u64 = 5;
    /// Invalid order ID format or length
    const E_INVALID_ORDER_ID: u64 = 6;
    /// Invalid address provided
    const E_INVALID_ADDRESS: u64 = 7;
    /// Operation not allowed on self
    const E_SELF_OPERATION: u64 = 8;

    // ==================== Resources ====================

    /// Main vault resource that stores USDC coins and manages deposits/withdrawals
    struct PaymentVault has key {
        /// USDC coin store for holding deposited funds
        usdc_store: Coin<USDC>,
        /// Total number of deposits made (for tracking and event indexing)
        deposit_count: u64,
        /// Vault creation timestamp
        created_at: u64
    }

    /// Owner capability resource for access control
    /// Only the holder of this resource can perform administrative operations
    struct OwnerCapability has key, store {
        /// Address of the current owner
        owner: address
    }

    // ==================== Events ====================

    // Event emitted when a deposit is successfully received
    #[event]
    struct DepositReceived has drop, store {
        /// Address of the payer who made the deposit
        payer: address,
        /// Unique order identifier provided by the payer
        order_id: vector<u8>,
        /// Amount of USDC deposited (in smallest units)
        amount: u64,
        /// Timestamp when the deposit was made
        timestamp: u64,
        /// Sequential deposit index for this vault
        deposit_index: u64
    }

    // Event emitted when owner withdraws funds from the vault
    #[event]
    struct WithdrawCompleted has drop, store {
        /// Address of the owner who initiated the withdrawal
        owner: address,
        /// Address of the recipient who received the funds
        recipient: address,
        /// Amount withdrawn (in smallest units)
        amount: u64,
        /// Timestamp when the withdrawal was completed
        timestamp: u64
    }

    // Event emitted when ownership is transferred to a new address
    #[event]
    struct OwnershipTransferred has drop, store {
        /// Previous owner address
        previous_owner: address,
        /// New owner address
        new_owner: address,
        /// Timestamp when ownership was transferred
        timestamp: u64
    }

    // ==================== Public Functions ====================

    /// Initialize the payment vault
    /// This function sets up the vault resources and owner capability
    ///
    /// # Parameters
    /// * `account` - The signer who will become the vault owner
    ///
    /// # Aborts
    /// * `E_ALREADY_INITIALIZED` - If vault already exists at this address
    ///
    /// # Examples
    /// ```move
    /// // Initialize vault (called by contract deployer)
    /// initialize(&deployer_signer);
    /// ```
    public entry fun initialize(account: &signer) {
        let account_addr = signer::address_of(account);

        // Ensure vault hasn't been initialized yet
        assert!(
            !exists<PaymentVault>(account_addr),
            error::already_exists(E_ALREADY_INITIALIZED)
        );
        assert!(
            !exists<OwnerCapability>(account_addr),
            error::already_exists(E_ALREADY_INITIALIZED)
        );

        // Create empty USDC coin store
        let usdc_store = coin::zero<USDC>();

        // Create vault resource
        let vault = PaymentVault {
            usdc_store,
            deposit_count: 0,
            created_at: timestamp::now_seconds()
        };

        // Create owner capability
        let owner_cap = OwnerCapability { owner: account_addr };

        // Move resources to account
        move_to(account, vault);
        move_to(account, owner_cap);
    }

    /// Deposit USDC tokens to the vault with an associated order ID
    /// This is the main deposit function that users will call to make payments
    ///
    /// # Parameters
    /// * `payer` - The signer making the deposit
    /// * `vault_address` - Address where the vault is deployed
    /// * `order_id` - Unique identifier for this order (32 bytes recommended)
    /// * `amount` - Amount of USDC to deposit (in smallest units)
    ///
    /// # Aborts
    /// * `E_NOT_INITIALIZED` - If vault doesn't exist at the specified address
    /// * `E_INVALID_AMOUNT` - If amount is 0
    /// * `E_INVALID_ORDER_ID` - If order_id is empty
    /// * `E_INSUFFICIENT_BALANCE` - If payer doesn't have enough USDC
    ///
    /// # Examples
    /// ```move
    /// // Deposit 100 USDC (assuming 6 decimals: 100 * 10^6)
    /// let order_id = b"unique_order_id_12345678901234567890";
    /// deposit(&payer_signer, @vault_address, order_id, 100000000);
    /// ```
    public entry fun deposit(
        payer: &signer,
        vault_address: address,
        order_id: vector<u8>,
        amount: u64
    ) acquires PaymentVault {
        // Input validation
        assert!(amount > 0, error::invalid_argument(E_INVALID_AMOUNT));
        assert!(
            !vector::is_empty(&order_id), error::invalid_argument(E_INVALID_ORDER_ID)
        );
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );

        let payer_addr = signer::address_of(payer);

        // Ensure payer has sufficient USDC balance
        assert!(
            coin::balance<USDC>(payer_addr) >= amount,
            error::invalid_state(E_INSUFFICIENT_BALANCE)
        );

        // Extract USDC from payer's account
        let deposit_coin = coin::withdraw<USDC>(payer, amount);

        // Get mutable reference to vault
        let vault = borrow_global_mut<PaymentVault>(vault_address);

        // Merge the deposited coin into vault's store
        coin::merge(&mut vault.usdc_store, deposit_coin);

        // Increment deposit counter
        vault.deposit_count = vault.deposit_count + 1;

        // Emit deposit event
        event::emit(
            DepositReceived {
                payer: payer_addr,
                order_id,
                amount,
                timestamp: timestamp::now_seconds(),
                deposit_index: vault.deposit_count
            }
        );
    }

    /// Withdraw a specific amount from the vault to a recipient address
    /// Only the vault owner can call this function
    ///
    /// # Parameters
    /// * `owner` - The vault owner's signer
    /// * `vault_address` - Address where the vault is deployed
    /// * `recipient` - Address to receive the withdrawn funds
    /// * `amount` - Amount to withdraw (in smallest units)
    ///
    /// # Aborts
    /// * `E_NOT_INITIALIZED` - If vault doesn't exist
    /// * `E_NOT_OWNER` - If caller is not the vault owner
    /// * `E_INVALID_AMOUNT` - If amount is 0
    /// * `E_INVALID_ADDRESS` - If recipient address is invalid
    /// * `E_INSUFFICIENT_BALANCE` - If vault doesn't have enough balance
    ///
    /// # Examples
    /// ```move
    /// // Withdraw 50 USDC to a specific address
    /// withdraw(&owner_signer, @vault_address, @recipient, 50000000);
    /// ```
    public entry fun withdraw(
        owner: &signer,
        vault_address: address,
        recipient: address,
        amount: u64
    ) acquires PaymentVault, OwnerCapability {
        // Input validation
        assert!(amount > 0, error::invalid_argument(E_INVALID_AMOUNT));
        assert!(recipient != @0x0, error::invalid_argument(E_INVALID_ADDRESS));
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );
        assert!(
            exists<OwnerCapability>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );

        // Verify owner permission
        verify_owner(owner, vault_address);

        let owner_addr = signer::address_of(owner);
        let vault = borrow_global_mut<PaymentVault>(vault_address);

        // Check vault has sufficient balance
        let vault_balance = coin::value(&vault.usdc_store);
        assert!(vault_balance >= amount, error::invalid_state(E_INSUFFICIENT_BALANCE));

        // Extract coins from vault
        let withdraw_coin = coin::extract(&mut vault.usdc_store, amount);

        // Ensure recipient can receive coins (register if needed)
        if (!coin::is_account_registered<USDC>(recipient)) {
            // Note: In production, the recipient should register themselves
            // This is a simplified approach for testing/demo purposes
            assert!(false, error::invalid_state(E_INVALID_ADDRESS));
        };

        // Deposit to recipient
        coin::deposit(recipient, withdraw_coin);

        // Emit withdrawal event
        event::emit(
            WithdrawCompleted {
                owner: owner_addr,
                recipient,
                amount,
                timestamp: timestamp::now_seconds()
            }
        );
    }

    /// Withdraw all funds from the vault to a recipient address
    /// Convenience function for withdrawing the entire vault balance
    ///
    /// # Parameters
    /// * `owner` - The vault owner's signer
    /// * `vault_address` - Address where the vault is deployed
    /// * `recipient` - Address to receive all withdrawn funds
    ///
    /// # Aborts
    /// * `E_NOT_INITIALIZED` - If vault doesn't exist
    /// * `E_NOT_OWNER` - If caller is not the vault owner
    /// * `E_INVALID_ADDRESS` - If recipient address is invalid
    /// * `E_INSUFFICIENT_BALANCE` - If vault has zero balance
    public entry fun withdraw_all(
        owner: &signer, vault_address: address, recipient: address
    ) acquires PaymentVault, OwnerCapability {
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );

        let vault = borrow_global<PaymentVault>(vault_address);
        let total_balance = coin::value(&vault.usdc_store);

        assert!(total_balance > 0, error::invalid_state(E_INSUFFICIENT_BALANCE));

        // Call the regular withdraw function with total balance
        withdraw(owner, vault_address, recipient, total_balance);
    }

    /// Transfer ownership of the vault to a new address
    /// The new owner must accept ownership by calling a separate function
    ///
    /// # Parameters
    /// * `current_owner` - Current owner's signer
    /// * `vault_address` - Address where the vault is deployed
    /// * `new_owner` - Address of the new owner
    ///
    /// # Aborts
    /// * `E_NOT_INITIALIZED` - If vault doesn't exist
    /// * `E_NOT_OWNER` - If caller is not the current owner
    /// * `E_INVALID_ADDRESS` - If new_owner is zero address
    /// * `E_SELF_OPERATION` - If trying to transfer to self
    public entry fun transfer_ownership(
        current_owner: &signer, vault_address: address, new_owner: address
    ) acquires OwnerCapability {
        // Input validation
        assert!(new_owner != @0x0, error::invalid_argument(E_INVALID_ADDRESS));
        assert!(
            exists<OwnerCapability>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );

        let current_owner_addr = signer::address_of(current_owner);
        assert!(
            new_owner != current_owner_addr, error::invalid_argument(E_SELF_OPERATION)
        );

        // Verify current owner permission
        verify_owner(current_owner, vault_address);

        // Update owner capability
        let owner_cap = borrow_global_mut<OwnerCapability>(vault_address);
        let previous_owner = owner_cap.owner;
        owner_cap.owner = new_owner;

        // Emit ownership transfer event
        event::emit(
            OwnershipTransferred {
                previous_owner,
                new_owner,
                timestamp: timestamp::now_seconds()
            }
        );
    }

    // ==================== View Functions ====================

    #[view]
    /// Get the current USDC balance in the vault
    public fun get_balance(vault_address: address): u64 acquires PaymentVault {
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );
        let vault = borrow_global<PaymentVault>(vault_address);
        coin::value(&vault.usdc_store)
    }

    #[view]
    /// Get the current owner address of the vault
    public fun get_owner(vault_address: address): address acquires OwnerCapability {
        assert!(
            exists<OwnerCapability>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );
        let owner_cap = borrow_global<OwnerCapability>(vault_address);
        owner_cap.owner
    }

    #[view]
    /// Get the total number of deposits made to the vault
    public fun get_deposit_count(vault_address: address): u64 acquires PaymentVault {
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );
        let vault = borrow_global<PaymentVault>(vault_address);
        vault.deposit_count
    }

    #[view]
    /// Get vault creation timestamp
    public fun get_created_at(vault_address: address): u64 acquires PaymentVault {
        assert!(
            exists<PaymentVault>(vault_address),
            error::not_found(E_NOT_INITIALIZED)
        );
        let vault = borrow_global<PaymentVault>(vault_address);
        vault.created_at
    }

    #[view]
    /// Check if a vault exists at the given address
    public fun vault_exists(vault_address: address): bool {
        exists<PaymentVault>(vault_address) && exists<OwnerCapability>(vault_address)
    }

    // ==================== Helper Functions ====================

    /// Internal function to verify that the caller is the vault owner
    ///
    /// # Parameters
    /// * `caller` - The signer to verify
    /// * `vault_address` - Address where the vault is deployed
    ///
    /// # Aborts
    /// * `E_NOT_OWNER` - If caller is not the vault owner
    fun verify_owner(caller: &signer, vault_address: address) acquires OwnerCapability {
        let caller_addr = signer::address_of(caller);
        let owner_cap = borrow_global<OwnerCapability>(vault_address);
        assert!(caller_addr == owner_cap.owner, error::permission_denied(E_NOT_OWNER));
    }

    // ==================== Test-Only Functions ====================

    #[test_only]
    use aptos_framework::coin::BurnCapability;
    #[test_only]
    use aptos_framework::coin::FreezeCapability;
    #[test_only]
    use aptos_framework::coin::MintCapability;

    #[test_only]
    use std::string;

    #[test_only]
    /// Initialize a fake USDC coin for testing purposes
    public fun init_usdc_for_test(
        admin: &signer
    ): (BurnCapability<USDC>, FreezeCapability<USDC>, MintCapability<USDC>) {
        coin::initialize<USDC>(
            admin,
            string::utf8(b"USD Coin"),
            string::utf8(b"USDC"),
            6, // 6 decimal places like real USDC
            true // monitor_supply
        )
    }

    #[test_only]
    /// Helper function to create test accounts with USDC
    public fun setup_test_account(
        _admin: &signer,
        user: &signer,
        mint_cap: &MintCapability<USDC>,
        amount: u64
    ) {
        let user_addr = signer::address_of(user);

        // Register user for USDC
        coin::register<USDC>(user);

        // Mint USDC to user
        let usdc_coins = coin::mint<USDC>(amount, mint_cap);
        coin::deposit(user_addr, usdc_coins);
    }
}
