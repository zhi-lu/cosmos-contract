use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouletteBetType {
    /// 单个数字下注 (0-36)
    SingleNumber { number: u8 },
    /// 红色/黑色
    Color { color: Color },
    /// 奇数/偶数
    EvenOdd { bet: EvenOdd },
    /// 大小下注
    HighLow { bet: HighLow },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Red,
    Black,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvenOdd {
    Even,
    Odd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HighLow {
    Low,  // 1-18
    High, // 19-36
}

/// 轮盘结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RouletteResult {
    pub winning_number: u8,
    pub winning_color: Color,
    pub is_even: bool,
    pub is_low: Option<bool>, // None for 0
}