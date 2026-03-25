use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// 刮刮乐（Scratch Card）数据类型
//
// 规则：
//   - 3×3 = 9 格的刮刮卡
//   - 每格随机分配一个奖励符号
//   - 中奖判定：
//     1. 任意一行（3 行）三个相同符号
//     2. 任意一列（3 列）三个相同符号
//     3. 对角线（2 条）三个相同符号
//     共 8 条中奖线
//   - 多条线同时中奖时叠加赔付
//
// 符号与赔率（含本金）：
//   💎 Diamond   → 50×
//   ⭐ Star      → 20×
//   🍀 Clover    → 10×
//   🔔 Bell      → 5×
//   🍒 Cherry    → 3×
//   🍋 Lemon     → 2×
//
// 卡面类型：
//   Classic  → 最低下注 100,000，最高 2,000,000
//   Premium  → 最低下注 500,000，最高 5,000,000
//   Deluxe   → 最低下注 1,000,000，最高 10,000,000
// ─────────────────────────────────────────────────────────────

/// 刮刮乐卡面类型
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScratchCardType {
    Classic,
    Premium,
    Deluxe,
}

/// 刮刮乐符号
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ScratchSymbol {
    Diamond, // 💎 最高奖
    Star,    // ⭐
    Clover,  // 🍀
    Bell,    // 🔔
    Cherry,  // 🍒
    Lemon,   // 🍋 最低奖
}

impl ScratchSymbol {
    /// 从随机数映射到符号（加权概率）
    /// Diamond 出现概率最低，Lemon 最高
    pub fn from_rand(val: u32) -> Self {
        match val % 100 {
            0..=2 => ScratchSymbol::Diamond,     // 3%
            3..=8 => ScratchSymbol::Star,        // 6%
            9..=18 => ScratchSymbol::Clover,     // 10%
            19..=33 => ScratchSymbol::Bell,      // 15%
            34..=55 => ScratchSymbol::Cherry,    // 22%
            _ => ScratchSymbol::Lemon,           // 44%
        }
    }

    /// 该符号中奖时的赔率倍数（含本金）
    pub fn multiplier(&self) -> u128 {
        match self {
            ScratchSymbol::Diamond => 50,
            ScratchSymbol::Star => 20,
            ScratchSymbol::Clover => 10,
            ScratchSymbol::Bell => 5,
            ScratchSymbol::Cherry => 3,
            ScratchSymbol::Lemon => 2,
        }
    }

    /// 符号显示名称
    pub fn name(&self) -> &'static str {
        match self {
            ScratchSymbol::Diamond => "Diamond",
            ScratchSymbol::Star => "Star",
            ScratchSymbol::Clover => "Clover",
            ScratchSymbol::Bell => "Bell",
            ScratchSymbol::Cherry => "Cherry",
            ScratchSymbol::Lemon => "Lemon",
        }
    }

    /// 符号 emoji
    pub fn emoji(&self) -> &'static str {
        match self {
            ScratchSymbol::Diamond => "💎",
            ScratchSymbol::Star => "⭐",
            ScratchSymbol::Clover => "🍀",
            ScratchSymbol::Bell => "🔔",
            ScratchSymbol::Cherry => "🍒",
            ScratchSymbol::Lemon => "🍋",
        }
    }
}

/// 获取卡面类型的下注范围
pub fn bet_range(card_type: &ScratchCardType) -> (u128, u128) {
    match card_type {
        ScratchCardType::Classic => (100_000, 2_000_000),
        ScratchCardType::Premium => (500_000, 5_000_000),
        ScratchCardType::Deluxe => (1_000_000, 10_000_000),
    }
}

/// 检查 8 条中奖线，返回所有中奖线的赔率倍数之和
/// 卡面布局（index 对应 3x3 格子）：
///   0 1 2
///   3 4 5
///   6 7 8
///
/// 中奖线：
///   行：[0,1,2], [3,4,5], [6,7,8]
///   列：[0,3,6], [1,4,7], [2,5,8]
///   对角线：[0,4,8], [2,4,6]
pub fn evaluate_scratch_card(grid: &[ScratchSymbol; 9]) -> (u128, Vec<(String, ScratchSymbol)>) {
    let lines: [(usize, usize, usize); 8] = [
        (0, 1, 2), // 行1
        (3, 4, 5), // 行2
        (6, 7, 8), // 行3
        (0, 3, 6), // 列1
        (1, 4, 7), // 列2
        (2, 5, 8), // 列3
        (0, 4, 8), // 对角线1
        (2, 4, 6), // 对角线2
    ];

    let line_names = [
        "row1", "row2", "row3",
        "col1", "col2", "col3",
        "diag1", "diag2",
    ];

    let mut total_multiplier: u128 = 0;
    let mut winning_lines: Vec<(String, ScratchSymbol)> = Vec::new();

    for (i, &(a, b, c)) in lines.iter().enumerate() {
        if grid[a] == grid[b] && grid[b] == grid[c] {
            let sym = grid[a];
            total_multiplier += sym.multiplier();
            winning_lines.push((line_names[i].to_string(), sym));
        }
    }

    (total_multiplier, winning_lines)
}

