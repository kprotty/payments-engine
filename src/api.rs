use serde::{Deserialize, Serialize};

pub type Currency = f64;

#[derive(Copy, Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub type TransactionId = u32;

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename = "tx")]
    pub id: TransactionId,
    #[serde(rename = "type")]
    pub operation: Operation,
    #[serde(rename = "client")]
    pub client_id: ClientId,
    pub amount: Option<Currency>,
}

pub type ClientId = u16;

#[derive(Debug, Serialize)]
pub struct Client {
    #[serde(rename = "client")]
    pub id: ClientId,
    #[serde(rename = "available")]
    pub balance: Currency,
    #[serde(rename = "held")]
    pub under_dispute: Currency,
    #[serde(rename = "total")]
    pub allocated: Currency,
    #[serde(rename = "locked")]
    pub is_frozen: bool,
}
