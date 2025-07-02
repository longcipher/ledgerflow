// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import "../src/PaymentVault.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

/**
 * @title Deploy script for upgradeable PaymentVault
 * @notice This script demonstrates how to deploy the PaymentVault with UUPS proxy pattern
 */
contract DeployUpgradeablePaymentVault is Script {
    PaymentVault public paymentVault;
    ERC1967Proxy public proxy;

    function run() external {
        vm.startBroadcast();

        // For testing purposes, using a mock USDC address
        // In production, use the actual USDC contract address
        address mockUSDC = address(0x1234567890123456789012345678901234567890);
        address initialOwner = msg.sender;

        // Deploy the implementation contract
        PaymentVault implementation = new PaymentVault();

        // Prepare the initialization data
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (mockUSDC, initialOwner));

        // Deploy the proxy pointing to the implementation
        proxy = new ERC1967Proxy(address(implementation), initData);

        // Cast the proxy address to PaymentVault for easier interaction
        paymentVault = PaymentVault(payable(address(proxy)));

        vm.stopBroadcast();

        console.log("Implementation deployed at:", address(implementation));
        console.log("Proxy deployed at:", address(proxy));
        console.log("PaymentVault accessible at:", address(paymentVault));
        console.log("Owner:", paymentVault.owner());
        console.log("USDC Token:", address(paymentVault.usdcToken()));
    }
}
