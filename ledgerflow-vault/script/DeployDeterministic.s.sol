// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import "../src/PaymentVault.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

/**
 * @title Deterministic deployment script for upgradeable PaymentVault
 * @notice This script uses CREATE2 to deploy PaymentVault with the same address across different chains
 * @dev The contract address will be the same on all EVM-compatible chains when using the same salt and deployer
 */
contract DeployDeterministicPaymentVault is Script {
    // Fixed salt for deterministic deployment - change this to get different addresses
    bytes32 public constant SALT = keccak256("PaymentVault_v1.0.0");

    PaymentVault public paymentVault;
    ERC1967Proxy public proxy;

    function run() external {
        vm.startBroadcast();

        // Get USDC token address from environment variable or use default
        address usdcToken = vm.envOr("USDC_TOKEN_ADDRESS", address(0));
        require(usdcToken != address(0), "USDC_TOKEN_ADDRESS environment variable must be set");

        // Get initial owner from environment variable or use deployer
        address initialOwner = vm.envOr("INITIAL_OWNER", msg.sender);

        // Deploy the implementation contract using CREATE2
        PaymentVault implementation = new PaymentVault{salt: SALT}();

        // Prepare the initialization data
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (usdcToken, initialOwner));

        // Deploy the proxy using CREATE2 with a different salt to avoid collision
        bytes32 proxySalt = keccak256(abi.encodePacked(SALT, "proxy"));
        proxy = new ERC1967Proxy{salt: proxySalt}(address(implementation), initData);

        // Cast the proxy address to PaymentVault for easier interaction
        paymentVault = PaymentVault(payable(address(proxy)));

        vm.stopBroadcast();

        console.log("=== Deployment Results ===");
        console.log("Chain ID:", block.chainid);
        console.log("Deployer:", msg.sender);
        console.log("Salt used:", vm.toString(SALT));
        console.log("Implementation deployed at:", address(implementation));
        console.log("Proxy deployed at:", address(proxy));
        console.log("PaymentVault accessible at:", address(paymentVault));
        console.log("Owner:", paymentVault.owner());
        console.log("USDC Token:", address(paymentVault.usdcToken()));
        console.log("Implementation version:", getImplementationVersion());
    }

    /**
     * @notice Predicts the deployment address before actual deployment
     * @dev This function can be called to check what address the contract will have
     * @param deployer The address that will deploy the contract
     * @param usdcToken The USDC token address for initialization
     * @param initialOwner The initial owner address
     * @return implementationAddr The predicted implementation address
     * @return proxyAddr The predicted proxy address
     */
    function predictAddresses(address deployer, address usdcToken, address initialOwner)
        external
        pure
        returns (address implementationAddr, address proxyAddr)
    {
        // Predict implementation address
        bytes memory implementationBytecode = abi.encodePacked(type(PaymentVault).creationCode);
        implementationAddr = calculateCreate2Address(SALT, keccak256(implementationBytecode), deployer);

        // Predict proxy address
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (usdcToken, initialOwner));
        bytes memory proxyBytecode =
            abi.encodePacked(type(ERC1967Proxy).creationCode, abi.encode(implementationAddr, initData));
        bytes32 proxySalt = keccak256(abi.encodePacked(SALT, "proxy"));
        proxyAddr = calculateCreate2Address(proxySalt, keccak256(proxyBytecode), deployer);
    }

    /**
     * @notice Gets a version identifier for the implementation
     * @dev This helps track which version is deployed
     */
    function getImplementationVersion() internal pure returns (string memory) {
        return "1.0.0";
    }

    /**
     * @notice Computes the CREATE2 address for a given salt and bytecode hash
     * @param salt The salt used for CREATE2
     * @param bytecodeHash The keccak256 hash of the bytecode
     * @param deployer The deployer address
     * @return The computed CREATE2 address
     */
    function calculateCreate2Address(bytes32 salt, bytes32 bytecodeHash, address deployer)
        internal
        pure
        returns (address)
    {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), deployer, salt, bytecodeHash)))));
    }

    /**
     * @notice Verifies that the deployment addresses match predictions
     * @dev Call this after deployment to ensure deterministic deployment worked
     */
    function verifyDeployment(address, /* expectedImpl */ address expectedProxy) external view {
        require(address(paymentVault) == expectedProxy, "Proxy address mismatch");
        console.log("Deployment verification passed");
    }
}
