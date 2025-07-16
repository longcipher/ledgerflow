diesel::table! {
    aptos_deposit_events (id) {
        id -> Int4,
        chain_id -> Varchar,
        contract_address -> Varchar,
        payer -> Varchar,
        order_id -> Varchar,
        amount -> Numeric,
        timestamp -> Timestamptz,
        deposit_index -> Int8,
        txn_version -> Int8,
        event_index -> Int4,
        created_at -> Nullable<Timestamptz>,
    }
}
