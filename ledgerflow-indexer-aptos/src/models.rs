use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::schema::aptos_deposit_events;

#[derive(Queryable, Insertable, Debug)]
#[diesel(table_name = aptos_deposit_events)]
pub struct AptosDepositEvent {
    pub chain_id: String,
    pub contract_address: String,
    pub payer: String,
    pub order_id: String,
    pub amount: BigDecimal,
    pub timestamp: DateTime<Utc>,
    pub deposit_index: i64,
    pub txn_version: i64,
    pub event_index: i32,
}
