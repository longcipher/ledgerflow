#[test_only]
module ledgerflow_vault_sui::payment_vault_tests_simple {
    use sui::test_scenario;
    use sui::coin;
    use sui::clock;
    use usdc::usdc::USDC;
    use ledgerflow_vault_sui::payment_vault;

    const ADMIN: address = @0xAD;
    const USER: address = @0xB0B;

    #[test]
    fun test_vault_creation_simple() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Verify initial state
        assert!(payment_vault::get_balance(&vault) == 0, 0);
        assert!(payment_vault::get_owner(&vault) == ADMIN, 1);
        assert!(payment_vault::get_deposit_count(&vault) == 0, 2);

        // Verify owner cap is bound to vault
        let vault_id = payment_vault::get_vault_id(&vault);
        let cap_vault_id = payment_vault::get_owner_cap_vault_id(&owner_cap);
        assert!(vault_id == cap_vault_id, 3);

        // Clean up - destroy objects directly
        payment_vault::destroy_vault_for_testing(vault);
        payment_vault::destroy_owner_cap_for_testing(owner_cap);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    fun test_deposit_simple() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Make deposit
        let usdc_coin = coin::mint_for_testing<USDC>(1000000, scenario.ctx()); // 1 USDC
        let order_id = b"test_order_123";

        payment_vault::deposit(&mut vault, usdc_coin, order_id, &clock, scenario.ctx());

        // Verify deposit
        assert!(payment_vault::get_balance(&vault) == 1000000, 0);
        assert!(payment_vault::get_deposit_count(&vault) == 1, 1);

        // Clean up
        payment_vault::destroy_vault_for_testing(vault);
        payment_vault::destroy_owner_cap_for_testing(owner_cap);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 3)]
    fun test_zero_amount_deposit_fails_simple() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Try zero amount deposit (should fail)
        let usdc_coin = coin::mint_for_testing<USDC>(0, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"test", &clock, scenario.ctx());

        // Clean up (won't be reached)
        payment_vault::destroy_vault_for_testing(vault);
        payment_vault::destroy_owner_cap_for_testing(owner_cap);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 4)]
    fun test_empty_order_id_fails_simple() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Try empty order ID (should fail)
        let usdc_coin = coin::mint_for_testing<USDC>(1000, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"", &clock, scenario.ctx());

        // Clean up (won't be reached)
        payment_vault::destroy_vault_for_testing(vault);
        payment_vault::destroy_owner_cap_for_testing(owner_cap);
        clock::destroy_for_testing(clock);
        scenario.end();
    }
}
