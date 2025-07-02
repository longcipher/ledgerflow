// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import "../src/PaymentVault.sol";
import "../test/PaymentVaultUpgrade.t.sol"; // Import PaymentVaultV2

/**
 * @title Upgrade script for PaymentVault
 * @notice This script demonstrates how to upgrade an existing PaymentVault to V2
 * @dev Before running this script:
 *      1. Deploy the initial PaymentVault using DeployUpgradeable.s.sol
 *      2. Note the proxy address from the deployment
 *      3. Replace PROXY_ADDRESS below with the actual proxy address
 *      4. Ensure you are the owner of the contract
 *      5. Test on testnet first before mainnet deployment
 */
contract UpgradePaymentVault is Script {
    // Replace with your actual proxy address after initial deployment
    address constant PROXY_ADDRESS = 0x1234567890123456789012345678901234567890;

    function run() external {
        vm.startBroadcast();

        // Deploy new implementation (V2)
        PaymentVaultV2 newImplementation = new PaymentVaultV2();

        // Get the existing proxy contract
        PaymentVault proxy = PaymentVault(payable(PROXY_ADDRESS));

        // Verify current owner can upgrade
        require(proxy.owner() == msg.sender, "Only owner can upgrade");

        // Perform upgrade with initialization
        proxy.upgradeToAndCall(address(newImplementation), abi.encodeCall(PaymentVaultV2.initializeV2, ()));

        vm.stopBroadcast();

        console.log("Upgraded PaymentVault to V2");
        console.log("New implementation:", address(newImplementation));
        console.log("Proxy address:", PROXY_ADDRESS);

        // Cast to V2 to verify upgrade
        PaymentVaultV2 vaultV2 = PaymentVaultV2(payable(PROXY_ADDRESS));
        console.log("Version after upgrade:", vaultV2.getVersion());
    }
}
