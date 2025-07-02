// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {PaymentVault} from "../src/PaymentVault.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

// Mock ERC20 contract for testing
contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "Insufficient balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        require(balanceOf[from] >= amount, "Insufficient balance");
        require(allowance[from][msg.sender] >= amount, "Insufficient allowance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        allowance[from][msg.sender] -= amount;
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }
}

contract PaymentVaultTest is Test {
    PaymentVault public paymentVault;
    MockERC20 public mockToken;
    address public owner;
    address public user;

    function setUp() public {
        owner = address(this);
        user = address(0x1);

        // Deploy mock USDC token
        mockToken = new MockERC20();

        // Deploy PaymentVault
        paymentVault = new PaymentVault(address(mockToken), owner);

        // Mint some tokens for testing
        mockToken.mint(user, 1000e6); // 1000 USDC
    }

    function test_Deposit() public {
        vm.startPrank(user);

        uint256 depositAmount = 100e6; // 100 USDC
        bytes32 orderId = keccak256("order123");

        // Approve PaymentVault to spend tokens
        mockToken.approve(address(paymentVault), depositAmount);

        // Perform deposit
        paymentVault.deposit(orderId, depositAmount);

        // Check that tokens were transferred
        assertEq(mockToken.balanceOf(address(paymentVault)), depositAmount);
        assertEq(mockToken.balanceOf(user), 1000e6 - depositAmount);

        vm.stopPrank();
    }

    function test_Withdraw() public {
        // First deposit some tokens
        vm.startPrank(user);
        uint256 depositAmount = 100e6;
        bytes32 orderId = keccak256("order123");
        mockToken.approve(address(paymentVault), depositAmount);
        paymentVault.deposit(orderId, depositAmount);
        vm.stopPrank();

        // Owner withdraws
        uint256 ownerBalanceBefore = mockToken.balanceOf(owner);
        paymentVault.withdraw();
        uint256 ownerBalanceAfter = mockToken.balanceOf(owner);

        assertEq(ownerBalanceAfter - ownerBalanceBefore, depositAmount);
        assertEq(mockToken.balanceOf(address(paymentVault)), 0);
    }

    function test_RevertOnZeroDeposit() public {
        vm.startPrank(user);
        bytes32 orderId = keccak256("order123");

        vm.expectRevert("Deposit amount must be greater than zero");
        paymentVault.deposit(orderId, 0);

        vm.stopPrank();
    }

    function test_RevertOnNativeTransfer() public {
        vm.expectRevert("Native currency transfers are not accepted");
        payable(address(paymentVault)).transfer(1 ether);
    }
}
