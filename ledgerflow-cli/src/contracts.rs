#![allow(clippy::too_many_arguments)]

use alloy::sol;

// Define PaymentVault contract interface
sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    interface PaymentVault {
        // Events
        event DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount);
        event WithdrawCompleted(address indexed owner, uint256 amount);

        // Functions
        function deposit(bytes32 orderId, uint256 amount) external;
        function depositWithPermit(
            bytes32 orderId,
            uint256 amount,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s
        ) external;
        function withdraw() external;
        function getBalance() external view returns (uint256);
        function owner() external view returns (address);
        function usdcToken() external view returns (address);
    }
}

// Define USDC token interface
sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    interface USDC {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function transfer(address to, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function permit(
            address owner,
            address spender,
            uint256 value,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s
        ) external;
        function nonces(address owner) external view returns (uint256);
        function DOMAIN_SEPARATOR() external view returns (bytes32);
    }
}
