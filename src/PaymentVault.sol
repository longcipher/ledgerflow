// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.30;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

/**
 * @title PaymentVault
 * @notice A secure upgradeable vault contract for handling USDC token deposits and withdrawals
 * @dev This contract implements a payment vault that accepts USDC deposits with order IDs
 *      and allows the owner to withdraw accumulated funds. It supports both standard
 *      ERC20 transfers and ERC-2612 permit-based transfers for improved UX.
 *      This contract is upgradeable using the UUPS (Universal Upgradeable Proxy Standard) pattern.
 * @author longcipher
 */
contract PaymentVault is Initializable, OwnableUpgradeable, UUPSUpgradeable {
    using SafeERC20 for IERC20;

    /// @notice The ERC20 token contract that this vault accepts (e.g., USDC)
    IERC20 public usdcToken;

    /// @notice The ERC20Permit interface for the USDC token to support permit functionality
    IERC20Permit public usdcPermitToken;

    /**
     * @notice Event emitted when a new deposit is successfully made.
     * @param payer The address that sent the funds.
     * @param orderId The unique identifier for the order, provided by the payer.
     * @param amount The amount of USDC tokens deposited.
     */
    event DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount);

    /**
     * @notice Event emitted when the owner successfully withdraws funds from the vault.
     * @param owner The address of the owner who withdrew the funds.
     * @param amount The amount of USDC tokens withdrawn.
     */
    event WithdrawCompleted(address indexed owner, uint256 amount);

    /**
     * @notice Event emitted when tokens are recovered from the vault.
     * @param token The address of the recovered token contract.
     * @param recipient The address that received the recovered tokens.
     * @param amount The amount of tokens recovered.
     */
    event TokenRecovered(address indexed token, address indexed recipient, uint256 amount);

    /**
     * @notice Initializes the vault with the specified USDC token address and initial owner.
     * @dev Replaces the constructor for upgradeable contracts. This function can only be called once.
     *      The initializer validates both token and owner addresses to prevent zero address assignments.
     *      Both usdcToken and usdcPermitToken point to the same contract address for gas efficiency.
     * @param _usdcTokenAddress The address of the USDC contract.
     * @param _initialOwner The initial owner of the contract (ideally a multi-sig wallet).
     */
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

    // ============ DEPOSIT FUNCTIONS ============

    /**
     * @notice Deposits a specified amount of USDC for a given order ID.
     * @dev The caller must have first approved this contract to spend at least `amount` of their USDC.
     *      This function performs balance checks before and after transfer to handle fee-on-transfer tokens
     *      properly, though USDC typically doesn't have transfer fees.
     * @param orderId The unique identifier for the order (e.g., a keccak256 hash of a string ID).
     * @param amount The amount of USDC to deposit.
     */
    function deposit(bytes32 orderId, uint256 amount) external {
        require(amount > 0, "Deposit amount must be greater than zero");

        // Transfer USDC from the caller to this contract.
        // This will revert if the caller has not approved enough tokens.
        uint256 beforeBalance = usdcToken.balanceOf(address(this));
        usdcToken.safeTransferFrom(msg.sender, address(this), amount);
        uint256 afterBalance = usdcToken.balanceOf(address(this));

        // A sanity check, especially for fee-on-transfer tokens.
        uint256 receivedAmount = afterBalance - beforeBalance;
        require(receivedAmount > 0, "Token transfer failed or received zero amount");

        // Emit an event for the off-chain indexer.
        emit DepositReceived(msg.sender, orderId, receivedAmount);
    }

    /**
     * @notice Deposits a specified amount of USDC for a given order ID using permit signature.
     * @dev This function uses ERC-2612 permit to approve and transfer in a single transaction,
     *      reducing gas costs by eliminating the need for a separate approve transaction.
     *      The permit signature must be valid and not expired. Balance checks are performed
     *      to handle potential fee-on-transfer tokens properly.
     * @param orderId The unique identifier for the order (e.g., a keccak256 hash of a string ID).
     * @param amount The amount of USDC to deposit.
     * @param deadline The deadline timestamp for the permit signature.
     * @param v The recovery byte of the signature.
     * @param r Half of the ECDSA signature pair.
     * @param s Half of the ECDSA signature pair.
     */
    function depositWithPermit(bytes32 orderId, uint256 amount, uint256 deadline, uint8 v, bytes32 r, bytes32 s)
        external
    {
        require(amount > 0, "Deposit amount must be greater than zero");
        require(deadline >= block.timestamp, "Permit signature has expired");

        // Execute the permit to approve the transfer
        usdcPermitToken.permit(msg.sender, address(this), amount, deadline, v, r, s);

        // Transfer USDC from the caller to this contract
        uint256 beforeBalance = usdcToken.balanceOf(address(this));
        usdcToken.safeTransferFrom(msg.sender, address(this), amount);
        uint256 afterBalance = usdcToken.balanceOf(address(this));

        // A sanity check, especially for fee-on-transfer tokens
        uint256 receivedAmount = afterBalance - beforeBalance;
        require(receivedAmount > 0, "Token transfer failed or received zero amount");

        // Emit an event for the off-chain indexer
        emit DepositReceived(msg.sender, orderId, receivedAmount);
    }

    // ============ WITHDRAWAL FUNCTIONS ============

    /**
     * @notice Withdraws the entire USDC balance of the contract to the owner.
     * @dev Only the owner can call this function. All accumulated USDC tokens in the vault
     *      will be transferred to the current owner address. This function will revert if
     *      there are no funds to withdraw.
     */
    function withdraw() external onlyOwner {
        uint256 balance = usdcToken.balanceOf(address(this));
        require(balance > 0, "No funds to withdraw");

        // Transfer all funds to the owner.
        usdcToken.safeTransfer(owner(), balance);

        // Emit an event for the off-chain indexer.
        emit WithdrawCompleted(owner(), balance);
    }

    /**
     * @notice Emergency function to recover any ERC20 tokens sent by mistake
     * @dev This function should NOT be used to withdraw USDC - use withdraw() instead.
     *      Only the contract owner can call this function. It prevents recovery of the main
     *      USDC token to avoid confusion with the primary withdrawal mechanism.
     * @param token The token contract address
     * @param recipient The address to receive the recovered tokens
     */
    function recoverToken(address token, address recipient) external onlyOwner {
        require(token != address(0), "Invalid token address");
        require(recipient != address(0), "Invalid recipient");
        require(token != address(usdcToken), "Use withdraw() for USDC recovery");

        IERC20 tokenContract = IERC20(token);
        uint256 balance = tokenContract.balanceOf(address(this));
        require(balance > 0, "No tokens to recover");

        // Use SafeERC20 for secure transfer
        tokenContract.safeTransfer(recipient, balance);

        // Emit an event for the off-chain indexer.
        emit TokenRecovered(token, recipient, balance);
    }

    // ============ VIEW FUNCTIONS ============

    /**
     * @notice Get current USDC balance in the vault
     * @dev This is a view function that returns the current USDC token balance held by this contract.
     *      Useful for off-chain monitoring and front-end display purposes.
     * @return The current USDC balance in the vault
     */
    function getBalance() external view returns (uint256) {
        return usdcToken.balanceOf(address(this));
    }

    // ============ SPECIAL FUNCTIONS ============

    receive() external payable {
        revert("Native currency transfers are not accepted");
    }

    /**
     * @notice Fallback function that rejects any calls to non-existent functions
     * @dev This provides additional security by explicitly rejecting any calls to
     *      functions that don't exist in this contract, preventing accidental loss of funds.
     */
    fallback() external payable {
        revert("Function does not exist");
    }

    // ============ UPGRADE FUNCTIONS ============

    /**
     * @notice Authorizes an upgrade to a new implementation
     * @dev This function is required by UUPSUpgradeable. Only the owner can authorize upgrades.
     * @param newImplementation The address of the new implementation contract
     */
    function _authorizeUpgrade(address newImplementation) internal override onlyOwner {
        // Only the owner can upgrade the contract
        // Additional upgrade validation logic can be added here if needed
    }
}
