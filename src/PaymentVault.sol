// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract PaymentVault is Ownable {
    // The ERC20 token contract that this vault accepts (e.g., USDC).
    IERC20 public immutable usdcToken;

    /**
     * @notice Event emitted when a new deposit is successfully made.
     * @param payer The address that sent the funds.
     * @param orderId The unique identifier for the order, provided by the payer.
     * @param amount The amount of USDC tokens deposited.
     */
    event DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount);

    /**
     * @notice Sets up the vault with the specified USDC token address and initial owner.
     * @param _usdcTokenAddress The address of the USDC contract.
     * @param _initialOwner The initial owner of the contract (ideally a multi-sig wallet).
     */
    constructor(address _usdcTokenAddress, address _initialOwner) Ownable(_initialOwner) {
        if (_usdcTokenAddress == address(0)) {
            revert("USDC token address cannot be the zero address.");
        }
        usdcToken = IERC20(_usdcTokenAddress);
    }

    /**
     * @notice Deposits a specified amount of USDC for a given order ID.
     * @dev The caller must have first approved this contract to spend at least `amount` of their USDC.
     * @param orderId The unique identifier for the order (e.g., a keccak256 hash of a string ID).
     * @param amount The amount of USDC to deposit.
     */
    function deposit(bytes32 orderId, uint256 amount) external {
        require(amount > 0, "Deposit amount must be greater than zero");

        // Transfer USDC from the caller to this contract.
        // This will revert if the caller has not approved enough tokens.
        uint256 beforeBalance = usdcToken.balanceOf(address(this));
        usdcToken.transferFrom(msg.sender, address(this), amount);
        uint256 afterBalance = usdcToken.balanceOf(address(this));

        // A sanity check, especially for fee-on-transfer tokens.
        uint256 receivedAmount = afterBalance - beforeBalance;
        require(receivedAmount > 0, "Token transfer failed or received zero amount");

        // Emit an event for the off-chain indexer.
        emit DepositReceived(msg.sender, orderId, receivedAmount);
    }

    /**
     * @notice Withdraws the entire USDC balance of the contract to the owner.
     * @dev Only the owner can call this function.
     */
    function withdraw() external onlyOwner {
        uint256 balance = usdcToken.balanceOf(address(this));
        require(balance > 0, "No funds to withdraw");

        // Transfer all funds to the owner.
        usdcToken.transfer(owner(), balance);
    }

    /**
     * @notice Rejects any accidental native currency (e.g., ETH, MATIC) transfers.
     */
    receive() external payable {
        revert("Native currency transfers are not accepted");
    }
}
