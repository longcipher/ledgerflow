// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {PaymentVault} from "../src/PaymentVault.sol";

contract PaymentVaultScript is Script {
    PaymentVault public paymentVault;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        // For testing purposes, using a mock USDC address
        // In production, use the actual USDC contract address
        address mockUSDC = address(0x1234567890123456789012345678901234567890);
        address initialOwner = msg.sender;

        paymentVault = new PaymentVault(mockUSDC, initialOwner);

        vm.stopBroadcast();
    }
}
