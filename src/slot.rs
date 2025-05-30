use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// 定义老虎机的符号
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Symbol {
    Apple,
    Origin,
    Cherry,
    Lemon,
    Bell,
    Seven,
    Bar,
}

impl Symbol {
    // 每种 slot 的概率
    pub fn from_u8(value: u32) -> Self {
        match value % 16 {
            0..=3 => Symbol::Apple,
            4..=6 => Symbol::Origin,
            7..=8 => Symbol::Cherry,
            9..=10 => Symbol::Lemon,
            11..=12 => Symbol::Bell,
            13..=14 => Symbol::Seven,
            _ => Symbol::Bar,
        }
    }

    // 每种 slot 的奖励
    pub fn payout_multiplier(&self) -> u64 {
        match self {
            Symbol::Apple => 2,
            Symbol::Origin => 3,
            Symbol::Cherry => 4,
            Symbol::Lemon => 4,
            Symbol::Bell => 4,
            Symbol::Seven => 8,
            Symbol::Bar => 16,
        }
    }
}
