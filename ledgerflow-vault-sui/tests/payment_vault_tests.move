#[test_only]
module ledgerflow_vault::payment_vault_tests {
    use sui::test_scenario;
    use sui::clock;
    use sui::transfer;
    use usdc::usdc::USDC;
    use ledgerflow_vault::payment_vault;
    use sui::coin;

    const ADMIN: address = @0xAD;
    const USER: address = @0xB0B;

    #[test]
    fun test_vault_creation() {
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

        // Clean up using transfer instead of return_to_sender
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    fun test_deposit_flow() {
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
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    fun test_withdrawal_flow() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault and make deposit
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());
        let usdc_coin = coin::mint_for_testing<USDC>(1000000, scenario.ctx());
        let order_id = b"test_order_123";
        payment_vault::deposit(&mut vault, usdc_coin, order_id, &clock, scenario.ctx());

        // Test withdrawal - entry functions don't return values, so we can't test return
        // Instead we test that the balance changes correctly
        let initial_balance = payment_vault::get_balance(&vault);
        payment_vault::withdraw(&mut vault, &owner_cap, 500000, ADMIN, &clock, scenario.ctx());
        let final_balance = payment_vault::get_balance(&vault);
        
        assert!(initial_balance == 1000000, 0);
        assert!(final_balance == 500000, 1);

        // Clean up
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test] 
    fun test_ownership_transfer() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Test ownership transfer
        let initial_owner = payment_vault::get_owner(&vault);
        payment_vault::transfer_ownership(&mut vault, &owner_cap, USER, &clock, scenario.ctx());
        let new_owner = payment_vault::get_owner(&vault);
        
        assert!(initial_owner == ADMIN, 0);
        assert!(new_owner == USER, 1);

        // Clean up
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, USER); // Transfer to new owner
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 2)]
    fun test_insufficient_balance_withdrawal_fails() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault with small balance
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());
        let usdc_coin = coin::mint_for_testing<USDC>(100, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"test", &clock, scenario.ctx());

        // Try to withdraw more than balance (should fail)
        payment_vault::withdraw(&mut vault, &owner_cap, 1000, ADMIN, &clock, scenario.ctx());

        // Clean up (won't be reached)
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 3)]
    fun test_zero_amount_deposit_fails() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Try zero amount deposit (should fail)
        let usdc_coin = coin::mint_for_testing<USDC>(0, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"test", &clock, scenario.ctx());

        // Clean up (won't be reached)
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 4)]
    fun test_empty_order_id_fails() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());

        // Try empty order ID (should fail)
        let usdc_coin = coin::mint_for_testing<USDC>(1000, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"", &clock, scenario.ctx());

        // Clean up (won't be reached)
        transfer::public_share_object(vault);
        transfer::public_transfer(owner_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = 7)]
    fun test_non_owner_withdrawal_fails() {
        let mut scenario = test_scenario::begin(ADMIN);
        let clock = clock::create_for_testing(scenario.ctx());

        // Create vault
        let (mut vault, owner_cap) = payment_vault::create_vault_for_testing(scenario.ctx());
        let usdc_coin = coin::mint_for_testing<USDC>(1000000, scenario.ctx());
        payment_vault::deposit(&mut vault, usdc_coin, b"test", &clock, scenario.ctx());

        // Create different vault with different cap
        let (vault2, fake_cap) = payment_vault::create_vault_for_testing(scenario.ctx());
        
        // This should fail - wrong capability for wrong vault
        payment_vault::withdraw(&mut vault, &fake_cap, 100, ADMIN, &clock, scenario.ctx());

        // Clean up (won't be reached due to expected failure)
        transfer::public_share_object(vault);
        transfer::public_share_object(vault2);
        transfer::public_transfer(owner_cap, ADMIN);
        transfer::public_transfer(fake_cap, ADMIN);
        clock::destroy_for_testing(clock);
        scenario.end();
    }
}
