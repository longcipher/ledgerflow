// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Test.sol";
import "../src/PaymentVault.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/**
 * @title Mock USDC contract for testing
 */
contract MockUSDC is ERC20 {
    constructor() ERC20("Mock USDC", "USDC") {
        _mint(msg.sender, 1000000 * 10 ** 6); // Mint 1M USDC with 6 decimals
    }

    function decimals() public pure override returns (uint8) {
        return 6;
    }
}

/**
 * @title Test for deterministic deployment
 * @notice This test verifies that the CREATE2 deployment produces consistent addresses
 */
contract TestDeterministicDeployment is Test {
    bytes32 public constant SALT = keccak256("PaymentVault_v1.0.0");

    address public constant INITIAL_OWNER = address(0xABcdEFABcdEFabcdEfAbCdefabcdeFABcDEFabCD);
    MockUSDC public mockUSDC;

    function setUp() public {
        mockUSDC = new MockUSDC();
    }

    function testDeterministicAddresses() public {
        // Test address prediction
        (address predictedImpl, address predictedProxy) = predictAddresses();

        // Deploy and verify predictions match
        (address actualImpl, address actualProxy) = deployWithCreate2();

        // Addresses should match predictions
        assertEq(predictedImpl, actualImpl, "Implementation prediction should match actual");
        assertEq(predictedProxy, actualProxy, "Proxy prediction should match actual");

        // Verify contracts work correctly
        PaymentVault vault = PaymentVault(payable(actualProxy));

        assertEq(vault.owner(), INITIAL_OWNER, "Vault owner should be correct");
        assertEq(address(vault.usdcToken()), address(mockUSDC), "Vault USDC should be correct");

        // Test that we can interact with the vault (now with real USDC mock)
        assertEq(vault.getBalance(), 0, "Initial balance should be zero");
    }

    function testAddressPrediction() public {
        // Predict addresses
        (address predictedImpl, address predictedProxy) = predictAddresses();

        // Deploy and verify predictions
        (address actualImpl, address actualProxy) = deployWithCreate2();

        assertEq(predictedImpl, actualImpl, "Implementation prediction should match");
        assertEq(predictedProxy, actualProxy, "Proxy prediction should match");
    }

    function deployWithCreate2() internal returns (address impl, address proxy) {
        // Deploy implementation with CREATE2
        PaymentVault implementation = new PaymentVault{salt: SALT}();
        impl = address(implementation);

        // Deploy proxy with CREATE2
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (address(mockUSDC), INITIAL_OWNER));
        bytes32 proxySalt = keccak256(abi.encodePacked(SALT, "proxy"));
        ERC1967Proxy proxyContract = new ERC1967Proxy{salt: proxySalt}(impl, initData);
        proxy = address(proxyContract);
    }

    function predictAddresses() internal view returns (address impl, address proxy) {
        address deployer = address(this);

        // Predict implementation address
        bytes memory implBytecode = abi.encodePacked(type(PaymentVault).creationCode);
        impl = calculateCreate2Address(SALT, keccak256(implBytecode), deployer);

        // Predict proxy address
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (address(mockUSDC), INITIAL_OWNER));
        bytes memory proxyBytecode = abi.encodePacked(type(ERC1967Proxy).creationCode, abi.encode(impl, initData));
        bytes32 proxySalt = keccak256(abi.encodePacked(SALT, "proxy"));
        proxy = calculateCreate2Address(proxySalt, keccak256(proxyBytecode), deployer);
    }

    function calculateCreate2Address(bytes32 salt, bytes32 bytecodeHash, address deployer)
        internal
        pure
        returns (address)
    {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), deployer, salt, bytecodeHash)))));
    }
}
