// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import {Test, console} from "forge-std/Test.sol";
import {PaymentVault} from "../src/PaymentVault.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC20Permit} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol";

// Mock ERC20 contract with permit functionality for testing
contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    mapping(address => uint256) public nonces;

    string public constant name = "Mock USDC";
    string public constant symbol = "MUSDC";
    uint8 public constant decimals = 6;

    // EIP-712 Domain Separator
    bytes32 private constant _DOMAIN_SEPARATOR = keccak256(
        abi.encode(
            keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
            keccak256(bytes(name)),
            keccak256(bytes("1")),
            1, // chainId for testing
            address(0) // will be set in constructor
        )
    );

    // Permit typehash
    bytes32 private constant PERMIT_TYPEHASH =
        keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)");

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

    function permit(
        address owner,
        address spender,
        uint256 value,
        uint256 deadline,
        uint8, /* v */
        bytes32, /* r */
        bytes32 /* s */
    ) external {
        require(deadline >= block.timestamp, "ERC20Permit: expired deadline");

        // For testing purposes, skip signature verification and just set allowance
        // In a real implementation, you would verify the signature properly
        allowance[owner][spender] = value;
        nonces[owner]++;
    }

    function DOMAIN_SEPARATOR() external pure returns (bytes32) {
        return _DOMAIN_SEPARATOR;
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

    function test_DepositWithPermit() public {
        vm.startPrank(user);

        uint256 depositAmount = 100e6; // 100 USDC
        bytes32 orderId = keccak256("order123");
        uint256 deadline = block.timestamp + 1 hours;

        // Perform deposit with permit
        paymentVault.depositWithPermit(
            orderId,
            depositAmount,
            deadline,
            27, // v
            bytes32(uint256(1)), // r
            bytes32(uint256(2)) // s
        );

        // Check that tokens were transferred
        assertEq(mockToken.balanceOf(address(paymentVault)), depositAmount);
        assertEq(mockToken.balanceOf(user), 1000e6 - depositAmount);

        vm.stopPrank();
    }

    function test_RevertOnExpiredPermit() public {
        vm.startPrank(user);

        uint256 depositAmount = 100e6; // 100 USDC
        bytes32 orderId = keccak256("order123");
        uint256 deadline = block.timestamp - 1; // Expired deadline

        vm.expectRevert("Permit signature has expired");
        paymentVault.depositWithPermit(
            orderId,
            depositAmount,
            deadline,
            27, // v
            bytes32(uint256(1)), // r
            bytes32(uint256(2)) // s
        );

        vm.stopPrank();
    }

    function test_RevertOnZeroDepositWithPermit() public {
        vm.startPrank(user);
        bytes32 orderId = keccak256("order123");
        uint256 deadline = block.timestamp + 1 hours;

        vm.expectRevert("Deposit amount must be greater than zero");
        paymentVault.depositWithPermit(
            orderId,
            0,
            deadline,
            27, // v
            bytes32(uint256(1)), // r
            bytes32(uint256(2)) // s
        );

        vm.stopPrank();
    }
}
