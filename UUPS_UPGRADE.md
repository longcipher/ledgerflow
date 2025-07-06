# LedgerFlow Vault UUPS Upgrade Implementation

This document explains the modifications made to the LedgerFlow Vault contract to support UUPS (Universal Upgradeable Proxy Standard) upgrades.

## Key Changes Made

### 1. Contract Inheritance
**Before:**
```solidity
contract PaymentVault is Ownable {
```

**After:**
```solidity
contract PaymentVault is Initializable, OwnableUpgradeable, UUPSUpgradeable {
```

### 2. Import Statements
**Added:**
```solidity
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
```

### 3. Storage Variables
**Before:**
```solidity
IERC20 public immutable usdcToken;
IERC20Permit public immutable usdcPermitToken;
```

**After:**
```solidity
IERC20 public usdcToken;
IERC20Permit public usdcPermitToken;
```

Note: `immutable` variables cannot be used in upgradeable contracts as they are stored in bytecode rather than storage.

### 4. Initialization Pattern
**Before (Constructor):**
```solidity
constructor(address _usdcTokenAddress, address _initialOwner) Ownable(_initialOwner) {
    require(_usdcTokenAddress != address(0), "USDC token address cannot be the zero address");
    require(_initialOwner != address(0), "Initial owner cannot be the zero address");
    usdcToken = IERC20(_usdcTokenAddress);
    usdcPermitToken = IERC20Permit(_usdcTokenAddress);
}
```

**After (Initializer):**
```solidity
function initialize(address _usdcTokenAddress, address _initialOwner) public initializer {
    require(_usdcTokenAddress != address(0), "USDC token address cannot be the zero address");
    require(_initialOwner != address(0), "Initial owner cannot be the zero address");
    
    // Initialize parent contracts
    __Ownable_init(_initialOwner);
    __UUPSUpgradeable_init();
    
    // Set storage variables
    usdcToken = IERC20(_usdcTokenAddress);
    usdcPermitToken = IERC20Permit(_usdcTokenAddress);
}
```

### 5. Upgrade Authorization
**Added:**
```solidity
function _authorizeUpgrade(address newImplementation) internal override onlyOwner {
    // Only the owner can upgrade the contract
    // Additional upgrade validation logic can be added here if needed
}
```

This function is required by UUPSUpgradeable and ensures only the contract owner can authorize upgrades.

## Deployment Pattern

### Standard Deployment (Non-Upgradeable)
```solidity
PaymentVault vault = new PaymentVault(usdcAddress, owner);
```

### Upgradeable Deployment (With Proxy)
```solidity
// Deploy implementation
PaymentVault implementation = new PaymentVault();

// Prepare initialization data
bytes memory initData = abi.encodeCall(
    PaymentVault.initialize,
    (usdcAddress, owner)
);

// Deploy proxy
ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);

// Use proxy address
PaymentVault vault = PaymentVault(payable(address(proxy)));
```

## Upgrade Process

To upgrade the contract:

1. Deploy new implementation contract
2. Call `upgradeToAndCall` on the proxy with the new implementation address
3. Optionally include initialization data for the new version

Example:
```solidity
// Deploy V2 implementation
PaymentVaultV2 newImplementation = new PaymentVaultV2();

// Upgrade with initialization
vault.upgradeToAndCall(
    address(newImplementation),
    abi.encodeCall(PaymentVaultV2.initializeV2, ())
);
```

## Security Considerations

1. **Owner Control**: Only the contract owner can authorize upgrades via `_authorizeUpgrade`
2. **Initialization Protection**: The `initializer` modifier ensures the contract can only be initialized once
3. **Proxy Pattern**: Uses ERC1967 proxy standard for secure upgrade functionality
4. **State Preservation**: All contract state is preserved during upgrades

## Testing

The implementation includes comprehensive tests for:
- Basic contract functionality remains unchanged
- Upgrade authorization (only owner can upgrade)
- State preservation during upgrades
- New functionality after upgrades
- Protection against double initialization

## Gas Considerations

UUPS upgrades are more gas-efficient than transparent proxies because:
- The upgrade logic is in the implementation contract, not the proxy
- Lower deployment cost for the proxy
- Slightly higher cost for the implementation due to upgrade logic

## Files Modified

1. `src/PaymentVault.sol` - Main contract with UUPS support
2. `script/PaymentVault.s.sol` - Updated deployment script
3. `test/PaymentVault.t.sol` - Updated test file
4. `script/DeployUpgradeable.s.sol` - New upgradeable deployment script
5. `test/PaymentVaultUpgrade.t.sol` - New upgrade functionality tests
6. `remappings.txt` - Added upgradeable contracts mapping

## Compatibility

The upgraded contract maintains full compatibility with the original interface. Existing integrations will continue to work without modifications.
