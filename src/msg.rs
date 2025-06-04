use crate::blackjack::BlackjackAction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // 比大小游戏
    PlayWar {},
    // 老虎机游戏
    PlaySlot {},
    // 猜数字游戏
    GuessNumber { guess: u8 },
    // 黑杰克游戏
    PlayBlackjack { action: BlackjackAction },
    // 部署者提款
    Withdraw { amount: u128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // 查询当前合约的锁仓金额
    GetLockedAmount {},
    
    // 查询某用户当前 Blackjack 游戏状态
    GetBlackjackState { address: String },
}
