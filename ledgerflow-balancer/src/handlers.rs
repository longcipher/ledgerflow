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
        AdminOrdersResponse, BalanceResponse, CreateOrderRequest, CreateOrderResponse,
        OrderResponse,
    },
    services::OrderService,
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
        vault_address: "0x0000000000000000000000000000000000000000".to_string(), /* TODO: Get from config */
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
    let (total_balance, completed_orders_count) =
        order_service.get_account_balance(account_id).await?;

    let response = BalanceResponse {
        account_id,
        total_balance,
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
