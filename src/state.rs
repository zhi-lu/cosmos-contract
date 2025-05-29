use cosmwasm_std::{Addr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,          // 合约所有者（部署者）
    pub locked_amount: u128,  // 锁仓金额
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LockedAmountResponse {
    pub locked_amount: Uint128,
}

pub const STATE: Item<State> = Item::new("state");