use crate::baccarat::BaccaratBet;
use crate::blackjack::BlackjackAction;
use crate::coin::CoinSide;
use crate::roulette::RouletteBetType;
use crate::slot::SlotMode;
use crate::omaha::OmahaAction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::dice::DiceGameMode;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // 比大小游戏
    PlayWar {},
    // 老虎机游戏（Basic = 3轮1线，Advanced = 5轮5线）
    PlaySlot { mode: SlotMode },
    // 猜数字游戏
    GuessNumber { guess: u8 },
    // 黑杰克游戏
    PlayBlackjack { action: BlackjackAction },
    // 硬币翻转游戏
    PlayCoinFlip { choice: CoinSide },
    // 骰子游戏
    PlayDice { mode: DiceGameMode },
    // 百家乐游戏
    PlayBaccarat { bet_choice: BaccaratBet },
    // 轮盘游戏
    PlayRoulette { bet_type: RouletteBetType },
    // 奥马哈扑克游戏（支持加注）
    PlayOmaha { action: OmahaAction },
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

    // 查询某用户当前 Omaha 游戏状态
    GetOmahaState { address: String },
}