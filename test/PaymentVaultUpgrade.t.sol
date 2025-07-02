// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Test.sol";
import "../src/PaymentVault.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

// Mock USDC token for testing
contract MockERC20 is ERC20 {
    constructor() ERC20("Mock USDC", "mUSDC") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function decimals() public pure override returns (uint8) {
        return 6; // USDC has 6 decimals
    }
}

// Upgraded version of PaymentVault for testing
contract PaymentVaultV2 is PaymentVault {
    uint256 public version;

    function initializeV2() public reinitializer(2) {
        version = 2;
    }

    function getVersion() external view returns (uint256) {
        return version;
    }

    function newFeature() external pure returns (string memory) {
        return "This is a new feature in V2";
    }
}

contract PaymentVaultUpgradeTest is Test {
    PaymentVault public paymentVault;
    PaymentVaultV2 public paymentVaultV2;
    ERC1967Proxy public proxy;
    MockERC20 public mockToken;

    address public owner;
    address public user;
    address public nonOwner;

    function setUp() public {
        owner = address(this);
        user = makeAddr("user");
        nonOwner = makeAddr("nonOwner");

        // Deploy mock USDC token
        mockToken = new MockERC20();

        // Deploy the implementation contract
        PaymentVault implementation = new PaymentVault();

        // Prepare the initialization data
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (address(mockToken), owner));

        // Deploy the proxy pointing to the implementation
        proxy = new ERC1967Proxy(address(implementation), initData);

        // Cast the proxy address to PaymentVault for easier interaction
        paymentVault = PaymentVault(payable(address(proxy)));

        // Mint some tokens for testing
        mockToken.mint(user, 1000e6); // 1000 USDC
    }

    function test_InitialDeployment() public view {
        assertEq(paymentVault.owner(), owner);
        assertEq(address(paymentVault.usdcToken()), address(mockToken));
        assertEq(paymentVault.getBalance(), 0);
    }

    function test_CannotInitializeTwice() public {
        vm.expectRevert(); // Use generic revert for compatibility
        paymentVault.initialize(address(mockToken), owner);
    }

    function test_UpgradeToV2() public {
        // Deploy V2 implementation
        PaymentVaultV2 implementationV2 = new PaymentVaultV2();

        // Perform upgrade
        paymentVault.upgradeToAndCall(address(implementationV2), abi.encodeCall(PaymentVaultV2.initializeV2, ()));

        // Cast to V2 to access new functionality
        paymentVaultV2 = PaymentVaultV2(payable(address(proxy)));

        // Verify upgrade succeeded
        assertEq(paymentVaultV2.getVersion(), 2);
        assertEq(paymentVaultV2.newFeature(), "This is a new feature in V2");

        // Verify existing functionality still works
        assertEq(paymentVaultV2.owner(), owner);
        assertEq(address(paymentVaultV2.usdcToken()), address(mockToken));
    }

    function test_OnlyOwnerCanUpgrade() public {
        PaymentVaultV2 implementationV2 = new PaymentVaultV2();

        vm.prank(nonOwner);
        vm.expectRevert();
        paymentVault.upgradeToAndCall(address(implementationV2), abi.encodeCall(PaymentVaultV2.initializeV2, ()));
    }

    function test_UpgradePreservesState() public {
        // Make a deposit first
        bytes32 orderId = keccak256("test-order");
        uint256 depositAmount = 100e6;

        vm.startPrank(user);
        mockToken.approve(address(paymentVault), depositAmount);
        paymentVault.deposit(orderId, depositAmount);
        vm.stopPrank();

        // Verify balance before upgrade
        assertEq(paymentVault.getBalance(), depositAmount);

        // Deploy V2 implementation and upgrade
        PaymentVaultV2 implementationV2 = new PaymentVaultV2();
        paymentVault.upgradeToAndCall(address(implementationV2), abi.encodeCall(PaymentVaultV2.initializeV2, ()));

        // Cast to V2
        paymentVaultV2 = PaymentVaultV2(payable(address(proxy)));

        // Verify state is preserved
        assertEq(paymentVaultV2.getBalance(), depositAmount);
        assertEq(paymentVaultV2.owner(), owner);
        assertEq(address(paymentVaultV2.usdcToken()), address(mockToken));

        // Verify new functionality works
        assertEq(paymentVaultV2.getVersion(), 2);
    }

    function test_UpgradeAndWithdraw() public {
        // Make a deposit
        bytes32 orderId = keccak256("test-order");
        uint256 depositAmount = 100e6;

        vm.startPrank(user);
        mockToken.approve(address(paymentVault), depositAmount);
        paymentVault.deposit(orderId, depositAmount);
        vm.stopPrank();

        // Upgrade to V2
        PaymentVaultV2 implementationV2 = new PaymentVaultV2();
        paymentVault.upgradeToAndCall(address(implementationV2), abi.encodeCall(PaymentVaultV2.initializeV2, ()));

        paymentVaultV2 = PaymentVaultV2(payable(address(proxy)));

        // Withdraw after upgrade
        uint256 ownerBalanceBefore = mockToken.balanceOf(owner);
        paymentVaultV2.withdraw();
        uint256 ownerBalanceAfter = mockToken.balanceOf(owner);

        assertEq(ownerBalanceAfter - ownerBalanceBefore, depositAmount);
        assertEq(paymentVaultV2.getBalance(), 0);
    }
}
