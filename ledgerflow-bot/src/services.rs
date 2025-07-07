#![allow(unused)]
use reqwest::Client;
use tracing::{error, info};

use crate::{
    config::Config,
    error::{BotError, BotResult},
    models::{BalanceResponse, CreateOrderRequest, CreateOrderResponse, Order},
};

pub struct BalancerService {
    client: Client,
    base_url: String,
}

impl BalancerService {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(
                config.balancer.timeout_seconds,
            ))
            .build()
            .unwrap();

        Self {
            client,
            base_url: config.balancer.base_url.clone(),
        }
    }

    pub async fn create_order(
        &self,
        request: CreateOrderRequest,
    ) -> BotResult<CreateOrderResponse> {
        let url = format!("{}/orders", self.base_url);

        info!("Creating order for account: {}", request.account_id);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!("Failed to create order: {}", error_text);
            return Err(BotError::BalancerApi(format!(
                "Failed to create order: {}",
                error_text
            )));
        }

        let order_response: CreateOrderResponse = response.json().await?;
        info!("Order created successfully: {}", order_response.order_id);

        Ok(order_response)
    }

    pub async fn get_order(&self, order_id: &str) -> BotResult<Order> {
        let url = format!("{}/orders/{}", self.base_url, order_id);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(BotError::BalancerApi(format!(
                "Failed to get order: {}",
                error_text
            )));
        }

        let order: Order = response.json().await?;
        Ok(order)
    }

    pub async fn get_balance(&self, account_id: &str) -> BotResult<BalanceResponse> {
        let url = format!("{}/accounts/{}/balance", self.base_url, account_id);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(BotError::BalancerApi(format!(
                "Failed to get balance: {}",
                error_text
            )));
        }

        let balance: BalanceResponse = response.json().await?;
        Ok(balance)
    }

    pub async fn health_check(&self) -> BotResult<bool> {
        let url = format!("{}/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
