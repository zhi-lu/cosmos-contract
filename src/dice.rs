use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiceGuessSize {
    Small,
    Big
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiceGameMode {
    /// 猜大小玩法（大：4-6，小：1-3）
    GuessSize {
        guess_big: DiceGuessSize,
    },

    /// 精准点数猜测（1~6）
    ExactNumber {
        guess_number: u8, // 必须是 1~6
    },

    /// 点数范围赌，例如 2~4
    RangeBet {
        start: u8, // 起始点（包含）
        end: u8,   // 结束点（包含）
    },
}
