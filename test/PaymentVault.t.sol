// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {PaymentVault} from "../src/PaymentVault.sol";

contract PaymentVaultTest is Test {
    PaymentVault public paymentVault;

    function setUp() public {
        paymentVault = new PaymentVault();
        paymentVault.setNumber(0);
    }

    function test_Increment() public {
        paymentVault.increment();
        assertEq(paymentVault.number(), 1);
    }

    function testFuzz_SetNumber(uint256 x) public {
        paymentVault.setNumber(x);
        assertEq(paymentVault.number(), x);
    }
}
