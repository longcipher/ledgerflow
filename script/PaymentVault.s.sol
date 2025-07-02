// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import {Script, console} from "forge-std/Script.sol";
import {PaymentVault} from "../src/PaymentVault.sol";

contract PaymentVaultScript is Script {
    PaymentVault public paymentVault;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        paymentVault = new PaymentVault();

        vm.stopBroadcast();
    }
}
