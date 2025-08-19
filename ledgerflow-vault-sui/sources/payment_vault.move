/// # LedgerFlow Payment Vault (Sui Version)
///
/// This module implements a secure payment vault for USDC deposits on Sui blockchain.
/// It provides non-custodial fund management with order tracking and owner-controlled withdrawals.
/// This version uses Sui's Coin standard for USDC interactions.
///
/// ## Core Features
/// - Secure USDC deposit with order ID tracking
/// - Owner-only withdrawal functionality
/// - Event emission for off-chain monitoring
/// - Ownership transfer capability
///
/// ## Architecture
/// The vault uses Sui's object model for enhanced security:
/// - PaymentVault object stores USDC coins and metadata
/// - OwnerCap object controls administrative functions
/// - Events provide real-time updates for indexers
///
/// ## Security Considerations
/// - All deposits are atomic operations
/// - Owner verification through capability-based access control
/// - Input validation on all public functions
/// - Object-based linear type safety prevents double-spending

module ledgerflow_vault::payment_vault {
    use sui::coin::{Self, Coin};
    use sui::balance::{Self, Balance};
    use sui::object;
    use sui::transfer;
    use sui::tx_context;
    use sui::event;
    use sui::clock;

    // ==================== Error Codes ====================

    /// Caller is not the owner
    const E_NOT_OWNER: u64 = 1;
    /// Insufficient balance for the operation
    const E_INSUFFICIENT_BALANCE: u64 = 2;
    /// Invalid amount (must be greater than 0)
    const E_INVALID_AMOUNT: u64 = 3;
    /// Invalid order ID format or length
    const E_INVALID_ORDER_ID: u64 = 4;
    /// Invalid address provided
    const E_INVALID_ADDRESS: u64 = 5;
    /// Operation not allowed on self
    const E_SELF_OPERATION: u64 = 6;
    /// Wrong vault ID for owner capability
    const E_WRONG_VAULT: u64 = 7;

    // ==================== Objects ====================

    /// Main vault object that stores USDC coins and manages deposits/withdrawals
    public struct PaymentVault<phantom T> has key, store {
        /// Unique identifier for the vault
        id: object::UID,
        /// USDC balance held by the vault (more efficient than Coin)
        usdc_balance: Balance<T>,
        /// Total number of deposits made (for tracking and event indexing)
        deposit_count: u64,
        /// Vault creation timestamp
        created_at: u64,
        /// Current owner address
        owner: address
    }

    /// Owner capability object for access control
    /// Only the holder of this object can perform administrative operations
    public struct OwnerCap has key, store {
        /// Unique identifier for the capability
        id: object::UID,
        /// ID of the vault this capability controls
        vault_id: object::ID
    }

    // ==================== Events ====================

    /// Event emitted when a deposit is successfully received
    public struct DepositReceived has copy, drop {
        /// ID of the vault that received the deposit
        vault_id: object::ID,
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

    /// Event emitted when owner withdraws funds from the vault
    public struct WithdrawCompleted has copy, drop {
        /// ID of the vault funds were withdrawn from
        vault_id: object::ID,
        /// Address of the owner who initiated the withdrawal
        owner: address,
        /// Address of the recipient who received the funds
        recipient: address,
        /// Amount withdrawn (in smallest units)
        amount: u64,
        /// Timestamp when the withdrawal was completed
        timestamp: u64
    }

    /// Event emitted when ownership is transferred to a new address
    public struct OwnershipTransferred has copy, drop {
        /// ID of the vault whose ownership was transferred
        vault_id: object::ID,
        /// Previous owner address
        previous_owner: address,
        /// New owner address
        new_owner: address,
        /// Timestamp when ownership was transferred
        timestamp: u64
    }

    // ==================== Public Functions ====================

    /// Initialize a payment vault
    /// This function creates a new vault and owner capability
    ///
    /// # Parameters
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    ///
    /// # Returns
    /// * Returns (PaymentVault, OwnerCap) tuple
    ///
    /// # Examples
    /// ```move
    /// let (vault, owner_cap) = payment_vault::init_vault(&clock, ctx);
    /// transfer::share_object(vault);
    /// transfer::transfer(owner_cap, tx_context::sender(ctx));
    /// ```
    public fun init_vault<T>(clock: &clock::Clock, ctx: &mut tx_context::TxContext): (PaymentVault<T>, OwnerCap) {
        let vault_uid = object::new(ctx);
        let vault_id = object::uid_to_inner(&vault_uid);
        let owner_addr = tx_context::sender(ctx);

        // Create vault with zero USDC balance
        let vault = PaymentVault {
            id: vault_uid,
            usdc_balance: balance::zero<T>(),
            deposit_count: 0,
            created_at: clock::timestamp_ms(clock),
            owner: owner_addr
        };

        // Create owner capability
        let owner_cap = OwnerCap {
            id: object::new(ctx),
            vault_id
        };

        (vault, owner_cap)
    }

    /// Initialize a payment vault and immediately share it
    /// This is a convenience function that creates and shares a vault in one transaction
    ///
    /// # Parameters
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    public entry fun create_shared_vault<T>(clock: &clock::Clock, ctx: &mut tx_context::TxContext) {
        let (vault, owner_cap) = init_vault<T>(clock, ctx);
        transfer::share_object(vault);
        transfer::transfer(owner_cap, tx_context::sender(ctx));
    }

    /// Deposit USDC tokens to the vault with an associated order ID
    /// This is the main deposit function that users will call to make payments
    ///
    /// # Parameters
    /// * `vault` - Mutable reference to the payment vault
    /// * `payment` - USDC coin to deposit
    /// * `order_id` - Unique identifier for this order (32 bytes recommended)
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    ///
    /// # Aborts
    /// * `E_INVALID_AMOUNT` - If payment amount is 0
    /// * `E_INVALID_ORDER_ID` - If order_id is empty
    ///
    /// # Examples
    /// ```move
    /// // Deposit 100 USDC (assuming 6 decimals: 100 * 10^6)
    /// let order_id = b"unique_order_id_12345678901234567890";
    /// deposit(&mut vault, usdc_coin, order_id, &clock, ctx);
    /// ```
    public entry fun deposit<T>(
        vault: &mut PaymentVault<T>,
        payment: Coin<T>,
        order_id: vector<u8>,
        clock: &clock::Clock,
        ctx: &mut tx_context::TxContext
    ) {
        let amount = coin::value(&payment);
        let payer_addr = tx_context::sender(ctx);

        // Input validation
        assert!(amount > 0, E_INVALID_AMOUNT);
        assert!(!std::vector::is_empty(&order_id), E_INVALID_ORDER_ID);

        // Convert coin to balance and add to vault balance
        let payment_balance = coin::into_balance(payment);
        balance::join(&mut vault.usdc_balance, payment_balance);

        // Increment deposit counter
        vault.deposit_count = vault.deposit_count + 1;

        // Emit deposit event
        event::emit(
            DepositReceived {
                vault_id: object::uid_to_inner(&vault.id),
                payer: payer_addr,
                order_id,
                amount,
                timestamp: clock::timestamp_ms(clock),
                deposit_index: vault.deposit_count
            }
        );
    }

    /// Withdraw a specific amount from the vault to a recipient address
    /// Only the vault owner can call this function
    ///
    /// # Parameters
    /// * `vault` - Mutable reference to the payment vault
    /// * `owner_cap` - Owner capability proving authorization
    /// * `amount` - Amount to withdraw (in smallest units)
    /// * `recipient` - Address to receive the withdrawn funds
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    ///
    /// # Aborts
    /// * `E_NOT_OWNER` - If caller doesn't own the capability
    /// * `E_WRONG_VAULT` - If capability is for a different vault
    /// * `E_INVALID_AMOUNT` - If amount is 0
    /// * `E_INVALID_ADDRESS` - If recipient address is invalid
    /// * `E_INSUFFICIENT_BALANCE` - If vault doesn't have enough balance
    ///
    /// # Examples
    /// ```move
    /// // Withdraw 50 USDC to a specific address
    /// withdraw(&mut vault, &owner_cap, 50000000, @recipient, &clock, ctx);
    /// ```
    public entry fun withdraw<T>(
        vault: &mut PaymentVault<T>,
        owner_cap: &OwnerCap,
        amount: u64,
        recipient: address,
        clock: &clock::Clock,
        ctx: &mut tx_context::TxContext
    ) {
        // Input validation
        assert!(amount > 0, E_INVALID_AMOUNT);
        assert!(recipient != @0x0, E_INVALID_ADDRESS);

        // Verify owner permission
        verify_owner(vault, owner_cap, ctx);

        let owner_addr = tx_context::sender(ctx);
        let vault_balance = balance::value(&vault.usdc_balance);

        // Check vault has sufficient balance
        assert!(vault_balance >= amount, E_INSUFFICIENT_BALANCE);

        // Split the requested amount from vault balance and convert to coin
        let withdrawal_balance = balance::split(&mut vault.usdc_balance, amount);
        let withdrawal_coin = coin::from_balance(withdrawal_balance, ctx);

        // Transfer to recipient
        transfer::public_transfer(withdrawal_coin, recipient);

        // Emit withdrawal event
        event::emit(
            WithdrawCompleted {
                vault_id: object::uid_to_inner(&vault.id),
                owner: owner_addr,
                recipient,
                amount,
                timestamp: clock::timestamp_ms(clock)
            }
        );
    }

    /// Withdraw all funds from the vault to a recipient address
    /// Convenience function for withdrawing the entire vault balance
    ///
    /// # Parameters
    /// * `vault` - Mutable reference to the payment vault
    /// * `owner_cap` - Owner capability proving authorization
    /// * `recipient` - Address to receive all withdrawn funds
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    ///
    /// # Aborts
    /// * `E_NOT_OWNER` - If caller doesn't own the capability
    /// * `E_WRONG_VAULT` - If capability is for a different vault
    /// * `E_INVALID_ADDRESS` - If recipient address is invalid
    /// * `E_INSUFFICIENT_BALANCE` - If vault has zero balance
    public entry fun withdraw_all<T>(
        vault: &mut PaymentVault<T>,
        owner_cap: &OwnerCap,
        recipient: address,
        clock: &clock::Clock,
        ctx: &mut tx_context::TxContext
    ) {
        let total_balance = balance::value(&vault.usdc_balance);
        assert!(total_balance > 0, E_INSUFFICIENT_BALANCE);

        // Call the regular withdraw function with total balance
        withdraw(vault, owner_cap, total_balance, recipient, clock, ctx);
    }

    /// Transfer ownership of the vault to a new address
    /// Updates the vault's owner field and emits an event
    ///
    /// # Parameters
    /// * `vault` - Mutable reference to the payment vault
    /// * `owner_cap` - Owner capability proving authorization
    /// * `new_owner` - Address of the new owner
    /// * `clock` - Clock object for timestamping
    /// * `ctx` - Transaction context
    ///
    /// # Aborts
    /// * `E_NOT_OWNER` - If caller doesn't own the capability
    /// * `E_WRONG_VAULT` - If capability is for a different vault
    /// * `E_INVALID_ADDRESS` - If new_owner is zero address
    /// * `E_SELF_OPERATION` - If trying to transfer to self
    public entry fun transfer_ownership<T>(
        vault: &mut PaymentVault<T>,
        owner_cap: &OwnerCap,
        new_owner: address,
        clock: &clock::Clock,
        ctx: &mut tx_context::TxContext
    ) {
        // Input validation
        assert!(new_owner != @0x0, E_INVALID_ADDRESS);

        let current_owner_addr = tx_context::sender(ctx);
        assert!(new_owner != current_owner_addr, E_SELF_OPERATION);

        // Verify current owner permission
        verify_owner(vault, owner_cap, ctx);

        // Update vault owner
        let previous_owner = vault.owner;
        vault.owner = new_owner;

        // Emit ownership transfer event
        event::emit(
            OwnershipTransferred {
                vault_id: object::uid_to_inner(&vault.id),
                previous_owner,
                new_owner,
                timestamp: clock::timestamp_ms(clock)
            }
        );
    }

    // ==================== View Functions ====================

    /// Get the current USDC balance in the vault
    public fun get_balance<T>(vault: &PaymentVault<T>): u64 {
        balance::value(&vault.usdc_balance)
    }

    /// Get the current owner address of the vault
    public fun get_owner<T>(vault: &PaymentVault<T>): address {
        vault.owner
    }

    /// Get the total number of deposits made to the vault
    public fun get_deposit_count<T>(vault: &PaymentVault<T>): u64 {
        vault.deposit_count
    }

    /// Get vault creation timestamp
    public fun get_created_at<T>(vault: &PaymentVault<T>): u64 {
        vault.created_at
    }

    /// Get the vault ID
    public fun get_vault_id<T>(vault: &PaymentVault<T>): object::ID {
        object::uid_to_inner(&vault.id)
    }

    /// Get the vault ID that this owner capability controls
    public fun get_owner_cap_vault_id(owner_cap: &OwnerCap): object::ID {
        owner_cap.vault_id
    }

    // ==================== Helper Functions ====================

    /// Internal function to verify that the caller is the vault owner
    /// and the capability is for the correct vault
    ///
    /// # Parameters
    /// * `vault` - Reference to the payment vault
    /// * `owner_cap` - Owner capability to verify
    /// * `ctx` - Transaction context
    ///
    /// # Aborts
    /// * `E_NOT_OWNER` - If caller is not the vault owner
    /// * `E_WRONG_VAULT` - If capability is for a different vault
    fun verify_owner<T>(vault: &PaymentVault<T>, owner_cap: &OwnerCap, ctx: &tx_context::TxContext) {
        let caller_addr = tx_context::sender(ctx);
        let vault_id = object::uid_to_inner(&vault.id);

        // Verify the caller is the vault owner
        assert!(caller_addr == vault.owner, E_NOT_OWNER);

        // Verify the capability is for this vault
        assert!(owner_cap.vault_id == vault_id, E_WRONG_VAULT);
    }

    // ==================== Test-Only Functions ====================

    #[test_only]
    /// Create a vault for testing purposes
    public fun create_vault_for_testing<T>(ctx: &mut tx_context::TxContext): (PaymentVault<T>, OwnerCap) {
        let vault_uid = object::new(ctx);
        let vault_id = object::uid_to_inner(&vault_uid);
        let owner_addr = tx_context::sender(ctx);

        let vault = PaymentVault {
            id: vault_uid,
            usdc_balance: balance::zero<T>(),
            deposit_count: 0,
            created_at: 0, // Use 0 for testing
            owner: owner_addr
        };

        let owner_cap = OwnerCap {
            id: object::new(ctx),
            vault_id
        };

        (vault, owner_cap)
    }

    #[test_only]
    /// Destroy a vault for testing purposes
    public fun destroy_vault_for_testing<T>(vault: PaymentVault<T>) {
        let PaymentVault {
            id,
            usdc_balance,
            deposit_count: _,
            created_at: _,
            owner: _
        } = vault;
        object::delete(id);
        balance::destroy_for_testing(usdc_balance);
    }

    #[test_only] 
    /// Destroy an owner capability for testing purposes
    public fun destroy_owner_cap_for_testing(owner_cap: OwnerCap) {
        let OwnerCap { id, vault_id: _ } = owner_cap;
        object::delete(id);
    }
}
