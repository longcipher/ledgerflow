// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "forge-std/Script.sol";
import "../src/PaymentVault.sol";
import "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

/**
 * @title Address prediction script for PaymentVault
 * @notice This script predicts the deployment addresses for PaymentVault across different chains
 * @dev Uses the same salt and deployer address to predict deterministic addresses
 */
contract PredictAddresses is Script {
    // Same salt as used in DeployDeterministic.s.sol
    bytes32 public constant SALT = keccak256("PaymentVault_v1.0.0");

    function run() external view {
        // Get deployer address from environment or use a default
        address deployer = vm.envOr("DEPLOYER_ADDRESS", address(0x1234567890123456789012345678901234567890));
        address usdcToken = vm.envOr("USDC_TOKEN_ADDRESS", address(0x1234567890123456789012345678901234567890));
        address initialOwner = vm.envOr("INITIAL_OWNER", deployer);

        console.log("=== PaymentVault Address Prediction ===");
        console.log("Deployer:", deployer);
        console.log("USDC Token:", usdcToken);
        console.log("Initial Owner:", initialOwner);
        console.log("Salt:", vm.toString(SALT));
        console.log("");

        // Predict implementation address
        bytes memory implementationBytecode = abi.encodePacked(type(PaymentVault).creationCode);
        address implementationAddr = calculateCreate2Address(SALT, keccak256(implementationBytecode), deployer);

        // Predict proxy address
        bytes memory initData = abi.encodeCall(PaymentVault.initialize, (usdcToken, initialOwner));
        bytes memory proxyBytecode =
            abi.encodePacked(type(ERC1967Proxy).creationCode, abi.encode(implementationAddr, initData));
        bytes32 proxySalt = keccak256(abi.encodePacked(SALT, "proxy"));
        address proxyAddr = calculateCreate2Address(proxySalt, keccak256(proxyBytecode), deployer);

        console.log("=== Predicted Addresses ===");
        console.log("Implementation:", implementationAddr);
        console.log("Proxy (PaymentVault):", proxyAddr);
        console.log("");
        console.log("=== Important Notes ===");
        console.log("1. These addresses will be the SAME on ALL EVM-compatible chains");
        console.log("2. The deployer address must be the same across all chains");
        console.log("3. The deployer must have the same nonce state on all chains");
        console.log("4. Use the same USDC token address and initial owner for consistency");
        console.log("");
        console.log("=== Deployment Command Preview ===");
        console.log("forge script script/DeployDeterministic.s.sol \\");
        console.log("  --rpc-url <RPC_URL> \\");
        console.log("  --private-key $PRIVATE_KEY \\");
        console.log("  --broadcast \\");
        console.log("  --verify");
    }

    /**
     * @notice Computes CREATE2 address
     */
    function calculateCreate2Address(bytes32 salt, bytes32 bytecodeHash, address deployer)
        internal
        pure
        returns (address)
    {
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), deployer, salt, bytecodeHash)))));
    }
}
