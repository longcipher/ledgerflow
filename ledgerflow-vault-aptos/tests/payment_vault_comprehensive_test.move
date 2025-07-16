#[test_only]
module ledgerflow_vault::payment_vault_comprehensive_test {
    use aptos_framework::coin;
    use aptos_framework::account;
    use aptos_framework::timestamp as ts;

    use ledgerflow_vault::payment_vault::{Self, USDC};

    // Test accounts
    const ADMIN: address = @0x1;
    const VAULT_OWNER: address = @0x2;
    const USER1: address = @0x3;
    const USER2: address = @0x4;
    const RECIPIENT: address = @0x5;

    // Test constants
    const INITIAL_USDC_AMOUNT: u64 = 1000000000; // 1000 USDC (6 decimals)
    const DEPOSIT_AMOUNT: u64 = 100000000; // 100 USDC
    const SMALL_AMOUNT: u64 = 1000000; // 1 USDC

    #[test]
    /// Test payment vault initialization and basic functionality
    fun test_payment_vault_basic() {
        // Create test accounts
        let admin = account::create_account_for_test(ADMIN);
        let vault_owner = account::create_account_for_test(VAULT_OWNER);
        let user1 = account::create_account_for_test(USER1);

        // Initialize timestamp for testing
        ts::set_time_has_started_for_testing(&admin);

        // Initialize USDC coin for testing
        let (burn_cap, freeze_cap, mint_cap) = payment_vault::init_usdc_for_test(&admin);

        // Setup user account with USDC
        payment_vault::setup_test_account(&admin, &user1, &mint_cap, INITIAL_USDC_AMOUNT);

        // Test vault initialization
        payment_vault::initialize(&vault_owner);
        assert!(payment_vault::vault_exists(VAULT_OWNER), 1);
        assert!(payment_vault::get_balance(VAULT_OWNER) == 0, 2);
        assert!(payment_vault::get_owner(VAULT_OWNER) == VAULT_OWNER, 3);
        assert!(payment_vault::get_deposit_count(VAULT_OWNER) == 0, 4);
        assert!(payment_vault::get_created_at(VAULT_OWNER) > 0, 5);

        // Test successful deposit
        let order_id = b"test_order_12345678901234567890123";
        let initial_user1_balance = coin::balance<USDC>(USER1);
        payment_vault::deposit(&user1, VAULT_OWNER, order_id, DEPOSIT_AMOUNT);
        assert!(payment_vault::get_balance(VAULT_OWNER) == DEPOSIT_AMOUNT, 6);
        assert!(payment_vault::get_deposit_count(VAULT_OWNER) == 1, 7);
        assert!(
            coin::balance<USDC>(USER1) == initial_user1_balance - DEPOSIT_AMOUNT,
            8
        );

        // Test withdrawal
        let recipient = account::create_account_for_test(RECIPIENT);
        coin::register<USDC>(&recipient);
        payment_vault::withdraw(
            &vault_owner,
            VAULT_OWNER,
            RECIPIENT,
            SMALL_AMOUNT
        );
        assert!(
            payment_vault::get_balance(VAULT_OWNER) == DEPOSIT_AMOUNT - SMALL_AMOUNT,
            9
        );
        assert!(coin::balance<USDC>(RECIPIENT) == SMALL_AMOUNT, 10);

        // Test withdraw all
        let remaining_balance = payment_vault::get_balance(VAULT_OWNER);
        payment_vault::withdraw_all(&vault_owner, VAULT_OWNER, RECIPIENT);
        assert!(payment_vault::get_balance(VAULT_OWNER) == 0, 11);
        assert!(
            coin::balance<USDC>(RECIPIENT) == SMALL_AMOUNT + remaining_balance,
            12
        );

        // Test ownership transfer
        payment_vault::deposit(
            &user1,
            VAULT_OWNER,
            b"replenish_123456789012345",
            SMALL_AMOUNT
        );
        let old_owner = payment_vault::get_owner(VAULT_OWNER);
        payment_vault::transfer_ownership(&vault_owner, VAULT_OWNER, @0x6);
        assert!(payment_vault::get_owner(VAULT_OWNER) == @0x6, 13);
        assert!(payment_vault::get_owner(VAULT_OWNER) != old_owner, 14);

        // Test view functions
        assert!(payment_vault::vault_exists(VAULT_OWNER) == true, 15);
        assert!(payment_vault::vault_exists(@0x999) == false, 16);

        // Clean up capabilities
        coin::destroy_burn_cap(burn_cap);
        coin::destroy_freeze_cap(freeze_cap);
        coin::destroy_mint_cap(mint_cap);
    }
}
