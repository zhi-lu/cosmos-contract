use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// 骰宝（Sic Bo / 大小）数据类型
//
// 规则：
//   - 摇三颗骰子，每颗点数 1-6
//   - 多种投注方式：
//     1. 大小 (Big/Small)：总和 4-10 = 小，11-17 = 大（三同号通杀）
//     2. 单双 (Odd/Even)：总和奇数/偶数（三同号通杀）
//     3. 总和 (Total)：猜三颗骰子的总和（4-17）
//     4. 三同号通选 (AnyTriple)：三颗一样，不指定点数
//     5. 指定三同号 (SpecificTriple)：指定三颗一样的点数
//     6. 双骰组合 (DoubleBet)：至少两颗骰子为指定点数
//     7. 单骰 (SingleDie)：指定点数出现次数越多赔越多
//     8. 两骰组合 (Combo)：指定两个不同点数各出现至少一次
//
// 赔率（含本金）：
//   - 大/小：2×
//   - 单/双：2×
//   - 总和：根据概率 7× ~ 61×
//   - 三同号通选：31×
//   - 指定三同号：181×
//   - 双骰：12×
//   - 单骰：出现 1 次 2×，2 次 3×，3 次 4×
//   - 两骰组合：7×
// ─────────────────────────────────────────────────────────────

/// 骰宝投注类型
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SicBoBetType {
    /// 大：总和 11-17（三同号除外）
    Big,
    /// 小：总和 4-10（三同号除外）
    Small,
    /// 单：总和为奇数（三同号除外）
    Odd,
    /// 双：总和为偶数（三同号除外）
    Even,
    /// 猜总和（4-17）
    Total { value: u8 },
    /// 三同号通选：三颗一样即中，不指定点数
    AnyTriple,
    /// 指定三同号：三颗都是指定的点数（1-6）
    SpecificTriple { number: u8 },
    /// 双骰：至少两颗骰子为指定点数（1-6）
    DoubleBet { number: u8 },
    /// 单骰：指定点数（1-6），按出现次数赔付
    SingleDie { number: u8 },
    /// 两骰组合：指定两个不同点数（各出现至少一次）
    Combo { first: u8, second: u8 },
}

/// 骰宝开骰结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SicBoResult {
    pub die1: u8,
    pub die2: u8,
    pub die3: u8,
    pub total: u8,
    pub is_triple: bool,
}

impl SicBoResult {
    pub fn new(d1: u8, d2: u8, d3: u8) -> Self {
        let total = d1 + d2 + d3;
        let is_triple = d1 == d2 && d2 == d3;
        SicBoResult {
            die1: d1,
            die2: d2,
            die3: d3,
            total,
            is_triple,
        }
    }

    /// 指定点数在三颗骰子中出现的次数
    pub fn count_of(&self, n: u8) -> u8 {
        let mut c = 0;
        if self.die1 == n { c += 1; }
        if self.die2 == n { c += 1; }
        if self.die3 == n { c += 1; }
        c
    }
}

/// 计算投注结果：返回 (是否中奖, 赔率倍数含本金)
pub fn calculate_sicbo_payout(bet: &SicBoBetType, result: &SicBoResult) -> (bool, u128) {
    match bet {
        // ── 大/小 ──────────────────────────────────
        SicBoBetType::Big => {
            // 总和 11-17 且非三同号
            let won = !result.is_triple && result.total >= 11 && result.total <= 17;
            (won, 2)
        }
        SicBoBetType::Small => {
            // 总和 4-10 且非三同号
            let won = !result.is_triple && result.total >= 4 && result.total <= 10;
            (won, 2)
        }

        // ── 单/双 ──────────────────────────────────
        SicBoBetType::Odd => {
            let won = !result.is_triple && result.total % 2 == 1;
            (won, 2)
        }
        SicBoBetType::Even => {
            let won = !result.is_triple && result.total % 2 == 0;
            (won, 2)
        }

        // ── 猜总和 ────────────────────────────────
        SicBoBetType::Total { value } => {
            let won = result.total == *value;
            // 赔率表（含本金）：
            let multiplier = match value {
                4 | 17 => 61,   // 概率 1/216
                5 | 16 => 31,   // 概率 3/216
                6 | 15 => 18,   // 概率 6/216
                7 | 14 => 13,   // 概率 10/216
                8 | 13 => 9,    // 概率 15/216
                9 | 12 => 7,    // 概率 21/216
                10 | 11 => 7,   // 概率 25/216
                _ => 0,
            };
            (won, multiplier)
        }

        // ── 三同号通选 ────────────────────────────
        SicBoBetType::AnyTriple => {
            (result.is_triple, 31)
        }

        // ── 指定三同号 ────────────────────────────
        SicBoBetType::SpecificTriple { number } => {
            let won = result.is_triple && result.die1 == *number;
            (won, 181)
        }

        // ── 双骰 ──────────────────────────────────
        SicBoBetType::DoubleBet { number } => {
            let won = result.count_of(*number) >= 2;
            (won, 12)
        }

        // ── 单骰 ──────────────────────────────────
        SicBoBetType::SingleDie { number } => {
            let count = result.count_of(*number);
            match count {
                1 => (true, 2),    // 出现 1 次：2×
                2 => (true, 3),    // 出现 2 次：3×
                3 => (true, 4),    // 出现 3 次：4×
                _ => (false, 0),   // 未出现
            }
        }

        // ── 两骰组合 ──────────────────────────────
        SicBoBetType::Combo { first, second } => {
            let has_first = result.count_of(*first) >= 1;
            let has_second = result.count_of(*second) >= 1;
            let won = has_first && has_second && first != second;
            (won, 7)
        }
    }
}

/// 验证投注类型参数是否合法
pub fn validate_bet(bet: &SicBoBetType) -> Result<(), &'static str> {
    match bet {
        SicBoBetType::Total { value } => {
            if *value < 4 || *value > 17 {
                return Err("Total bet must be between 4 and 17");
            }
        }
        SicBoBetType::SpecificTriple { number }
        | SicBoBetType::DoubleBet { number }
        | SicBoBetType::SingleDie { number } => {
            if *number < 1 || *number > 6 {
                return Err("Die number must be between 1 and 6");
            }
        }
        SicBoBetType::Combo { first, second } => {
            if *first < 1 || *first > 6 || *second < 1 || *second > 6 {
                return Err("Combo numbers must be between 1 and 6");
            }
            if first == second {
                return Err("Combo numbers must be different");
            }
        }
        _ => {}
    }
    Ok(())
}

