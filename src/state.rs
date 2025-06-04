use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,         // 合约所有者（部署者）
    pub locked_amount: u128, // 锁仓金额
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BlackjackState {
    pub user_cards: Vec<u32>,   // 用户的牌
    pub dealer_cards: Vec<u32>, // 庄家牌
    pub bet: Uint128,           // 投注金额
    pub finished: bool,         // 是否结束
}

// 目前将 21 点返回的 Response 设置为 BlackjackState 类型.
pub(crate) type BlackjackStateResponse = BlackjackState;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LockedAmountResponse {
    pub locked_amount: Uint128,
}

// 锁仓状态
pub const STATE: Item<State> = Item::new("state");

// 21 点状态
pub const BLACKJACK_STATE: Map<&Addr, BlackjackState> = Map::new("blackjack_state");
