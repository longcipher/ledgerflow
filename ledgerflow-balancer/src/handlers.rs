use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::Deserialize;
use tracing::info;

use crate::{
    AppState,
    auth::AuthContext,
    error::AppError,
    models::{
        AccountResponse, AdminOrdersResponse, BalanceResponse, CreateOrderRequest,
        CreateOrderResponse, OrderResponse, RegisterAccountRequest, RegisterAccountResponse,
    },
    services::{AccountService, BalanceService, OrderService, RegisterAccountResult},
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn create_order(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, AppError> {
    ensure_can_access_account(&auth, request.account_id)?;
    info!("Creating order for account: {}", request.account_id);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );
    let order = order_service.create_order(request).await?;

    let order_id = if order.order_id.starts_with("0x") {
        order.order_id.clone()
    } else {
        format!("0x{}", order.order_id)
    };

    let response = CreateOrderResponse {
        order_id,
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
    auth: AuthContext,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>, AppError> {
    info!("Getting order: {}", order_id);

    let order_service = OrderService::new(
        (*state.db).clone(),
        state.config.business.max_pending_orders_per_account,
    );

    // Remove "0x" prefix if present
    let normalized_order_id = if order_id.starts_with("0x") {
        order_id.trim_start_matches("0x").to_string()
    } else {
        order_id.clone()
    };

    let order = order_service.get_order(&normalized_order_id).await?;
    ensure_can_access_account(&auth, order.account_id)?;

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
    auth: AuthContext,
    Path(account_id): Path<i64>,
) -> Result<Json<BalanceResponse>, AppError> {
    ensure_can_access_account(&auth, account_id)?;
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
    auth: AuthContext,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<AdminOrdersResponse>, AppError> {
    if !auth.is_admin {
        return Err(AppError::Forbidden("admin privileges required".to_string()));
    }

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
    let RegisterAccountResult { account, api_token } =
        account_service.register_account(request).await?;

    let response = RegisterAccountResponse {
        id: account.id,
        username: account.username,
        email: account.email,
        telegram_id: account.telegram_id,
        evm_address: account.evm_address,
        api_token,
        is_admin: account.is_admin,
        created_at: account.created_at,
        updated_at: account.updated_at,
    };

    info!("Account registered successfully with ID: {}", account.id);
    Ok(Json(response))
}

pub async fn get_account_by_username(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(username): Path<String>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by username: {}", username);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service.get_account_by_username(&username).await?;

    match account {
        Some(account) => {
            ensure_can_access_account(&auth, account.id)?;
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
    auth: AuthContext,
    Path(email): Path<String>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by email: {}", email);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service.get_account_by_email(&email).await?;

    match account {
        Some(account) => {
            ensure_can_access_account(&auth, account.id)?;
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
    auth: AuthContext,
    Path(telegram_id): Path<i64>,
) -> Result<Json<AccountResponse>, AppError> {
    info!("Getting account by telegram_id: {}", telegram_id);

    let account_service = AccountService::new((*state.db).clone());
    let account = account_service
        .get_account_by_telegram_id(telegram_id)
        .await?;

    match account {
        Some(account) => {
            ensure_can_access_account(&auth, account.id)?;
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

fn ensure_can_access_account(auth: &AuthContext, account_id: i64) -> Result<(), AppError> {
    if auth.is_admin {
        return Ok(());
    }

    if let Some(auth_account_id) = auth.account_id()
        && auth_account_id == account_id
    {
        return Ok(());
    }

    Err(AppError::Forbidden(
        "access denied for requested account".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::ensure_can_access_account;
    use crate::auth::{AuthContext, AuthPrincipal};

    #[test]
    fn admin_principal_can_access_any_account() {
        let auth = AuthContext {
            principal: AuthPrincipal::Service,
            is_admin: true,
        };
        assert!(ensure_can_access_account(&auth, 42).is_ok());
    }

    #[test]
    fn account_principal_can_access_own_account() {
        let auth = AuthContext {
            principal: AuthPrincipal::Account { id: 7 },
            is_admin: false,
        };
        assert!(ensure_can_access_account(&auth, 7).is_ok());
    }

    #[test]
    fn account_principal_cannot_access_other_account() {
        let auth = AuthContext {
            principal: AuthPrincipal::Account { id: 7 },
            is_admin: false,
        };
        assert!(ensure_can_access_account(&auth, 8).is_err());
    }
}
