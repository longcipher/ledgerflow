use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;
use tracing::info;

use crate::{
    AppState,
    error::AppError,
    models::{
        AccountResponse, AdminOrdersResponse, BalanceResponse, CreateOrderRequest,
        CreateOrderResponse, OrderResponse, RegisterAccountRequest, RegisterAccountResponse,
    },
    services::{AccountService, BalanceService, OrderService},
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_order(
    State(state): State<AppState>,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, AppError> {
    info!("Creating order for account: {}", request.account_id);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );
    let order = order_service.create_order(request).await?;

    let response = CreateOrderResponse {
        order_id: order.order_id.clone(),
        amount: order.amount,
        token_address: order.token_address,
        chain_id: order.chain_id,
        status: order.status,
        created_at: order.created_at,
    };

    info!("Order created successfully: {}", order.order_id);
    Ok(Json(response))
}

pub async fn get_order(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>, AppError> {
    info!("Getting order: {}", order_id);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );
    let order = order_service.get_order(&order_id).await?;

    let response = OrderResponse {
        order_id: order.order_id,
        account_id: order.account_id,
        amount: order.amount,
        token_address: order.token_address,
        chain_id: order.chain_id,
        status: order.status,
        created_at: order.created_at,
        updated_at: order.updated_at,
        transaction_hash: order.transaction_hash,
    };

    Ok(Json(response))
}

pub async fn get_balance(
    State(state): State<AppState>,
    Path(account_id): Path<i64>,
) -> Result<Json<BalanceResponse>, AppError> {
    info!("Getting balance for account: {}", account_id);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );
    let balance_service = BalanceService::new((*state.db).clone());

    let balance = balance_service.get_account_balance(account_id).await?;
    let completed_orders_count = order_service.get_completed_orders(account_id).await?;

    let response = BalanceResponse {
        account_id,
        total_balance: balance.balance,
        completed_orders_count: completed_orders_count as u32,
    };

    Ok(Json(response))
}

pub async fn list_pending_orders(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<AdminOrdersResponse>, AppError> {
    info!("Listing pending orders with pagination: {:?}", pagination);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );
    let orders = order_service
        .list_pending_orders(pagination.limit, pagination.offset)
        .await?;

    let order_responses: Vec<OrderResponse> = orders
        .into_iter()
        .map(|order| OrderResponse {
            order_id: order.order_id,
            account_id: order.account_id,
            amount: order.amount,
            token_address: order.token_address,
            chain_id: order.chain_id,
            status: order.status,
            created_at: order.created_at,
            updated_at: order.updated_at,
            transaction_hash: order.transaction_hash,
        })
        .collect();

    let response = AdminOrdersResponse {
        total_count: order_responses.len() as u32,
        orders: order_responses,
    };

    Ok(Json(response))
}

pub async fn register_account(
    State(state): State<AppState>,
    Json(request): Json<RegisterAccountRequest>,
) -> Result<Json<RegisterAccountResponse>, AppError> {
    info!(
        "Registering new account with username: {}",
        request.username
    );

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service.register_account(request).await?;

    let response = RegisterAccountResponse {
        id: account.id,
        username: account.username,
        email: account.email,
        telegram_id: account.telegram_id,
        evm_address: account.evm_address,
        is_admin: account.is_admin,
        created_at: account.created_at,
        updated_at: account.updated_at,
    };

    info!("Account registered successfully with ID: {}", account.id);
    Ok(Json(response))
}

pub async fn get_account_by_username(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by username: {}", username);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service.get_account_by_username(&username).await?;

    match account {
        Some(account) => {
            let response = AccountResponse {
                id: account.id,
                username: account.username,
                email: account.email,
                telegram_id: account.telegram_id,
                evm_address: account.evm_address,
                is_admin: account.is_admin,
                created_at: account.created_at,
                updated_at: account.updated_at,
            };
            Ok(Json(response))
        }
        None => Err(AppError::NotFound(format!(
            "Account with username {username} not found"
        ))),
    }
}

pub async fn get_account_by_email(
    State(state): State<AppState>,
    Path(email): Path<String>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by email: {}", email);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service.get_account_by_email(&email).await?;

    match account {
        Some(account) => {
            let response = AccountResponse {
                id: account.id,
                username: account.username,
                email: account.email,
                telegram_id: account.telegram_id,
                evm_address: account.evm_address,
                is_admin: account.is_admin,
                created_at: account.created_at,
                updated_at: account.updated_at,
            };
            Ok(Json(response))
        }
        None => Err(AppError::NotFound(format!(
            "Account with email {email} not found"
        ))),
    }
}

pub async fn get_account_by_telegram_id(
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by telegram_id: {}", telegram_id);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service
        .get_account_by_telegram_id(telegram_id)
        .await?;

    match account {
        Some(account) => {
            let response = AccountResponse {
                id: account.id,
                username: account.username,
                email: account.email,
                telegram_id: account.telegram_id,
                evm_address: account.evm_address,
                is_admin: account.is_admin,
                created_at: account.created_at,
                updated_at: account.updated_at,
            };
            Ok(Json(response))
        }
        None => Err(AppError::NotFound(format!(
            "Account with telegram_id {telegram_id} not found"
        ))),
    }
}
