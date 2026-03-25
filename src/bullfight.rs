use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// 斗牛（Bull Bull / Niu Niu）数据类型
//
// 规则：
//   - 使用一副 52 张标准扑克牌
//   - 每人发 5 张牌
//   - 点数计算：A=1, 2-9=面值, 10/J/Q/K=10
//   - 从 5 张中选 3 张使其点数之和为 10 的倍数（称为"有牛"）
//   - 剩余 2 张的点数之和取个位数即为"牛几"（0=牛牛=10 点）
//   - 若无法选出 3 张凑成 10 的倍数，则为"没牛"
//
// 特殊牌型（从高到低）：
//   1. 五小牛 (Wuxiaoniu)：5 张牌点数都≤5 且总和≤10       → 8×
//   2. 四炸   (SiZha/Bomb)：4 张相同点数                   → 7×
//   3. 五花牛 (Wuhuaniu)：5 张全是 J/Q/K（花牌）            → 6×
//   4. 牛牛   (NiuNiu)：有牛且剩余 2 张和为 10 的倍数       → 4×
//   5. 牛九   (Niu9)                                       → 3×
//   6. 牛八   (Niu8)                                       → 3×
//   7. 牛七   (Niu7)                                       → 2×
//   8. 牛一~牛六 (Niu1-Niu6)                               → 2×
//   9. 没牛   (NoNiu)                                      → 2×
//
// 平局时退还本金
// ─────────────────────────────────────────────────────────────

/// 花色
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub enum Suit {
    Spades,   // 黑桃 ♠
    Hearts,   // 红心 ♥
    Diamonds, // 方块 ♦
    Clubs,    // 梅花 ♣
}

impl Suit {
    pub fn from_u8(v: u8) -> Self {
        match v % 4 {
            0 => Suit::Spades,
            1 => Suit::Hearts,
            2 => Suit::Diamonds,
            _ => Suit::Clubs,
        }
    }
}

/// 斗牛的牌：rank 1=A, 2-10, 11=J, 12=Q, 13=K
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub struct BullCard {
    pub rank: u8, // 1..=13
    pub suit: Suit,
}

impl BullCard {
    /// 从 0..=51 的牌 ID 构造
    pub fn from_id(id: u8) -> Self {
        let rank = id / 4 + 1; // 0-51 => rank 1-13
        let suit = Suit::from_u8(id % 4);
        BullCard { rank, suit }
    }

    /// 斗牛点数：A=1, 2-9=面值, 10/J/Q/K=10
    pub fn point(&self) -> u8 {
        if self.rank >= 10 {
            10
        } else {
            self.rank
        }
    }

    /// 是否是花牌（J/Q/K）
    pub fn is_face(&self) -> bool {
        self.rank >= 11
    }
}

// ─────────────────────────────────────────────────────────────
// 牌型定义
// ─────────────────────────────────────────────────────────────

/// 斗牛牌型（从高到低）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum BullHandType {
    /// 五小牛：5 张牌点数都≤5 且总和≤10
    WuXiaoNiu,
    /// 四炸：4 张相同 rank
    SiZha { rank: u8 },
    /// 五花牛：5 张全是 J/Q/K
    WuHuaNiu,
    /// 牛牛：有牛且剩余 2 张和为 10 的倍数（即牛 10）
    NiuNiu,
    /// 牛 N（1-9）
    NiuN { n: u8 },
    /// 没牛
    NoNiu,
}

/// 斗牛手牌评估结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BullHandRank {
    pub hand_type: BullHandType,
    /// 综合分数（越大越强），用于比较
    pub score: u32,
    /// 最大单牌 rank（同牌型时比较用）
    pub max_rank: u8,
}

// ─────────────────────────────────────────────────────────────
// 手牌评估
// ─────────────────────────────────────────────────────────────

/// 评估 5 张牌的斗牛牌力
pub fn evaluate_bull_hand(cards: &[BullCard; 5]) -> BullHandRank {
    let max_rank = cards.iter().map(|c| c.rank).max().unwrap_or(0);

    // ── 1. 五小牛：5 张牌 rank 都≤5 且点数总和≤10 ──
    let all_small = cards.iter().all(|c| c.rank <= 5);
    let total_points: u8 = cards.iter().map(|c| c.point()).sum();
    if all_small && total_points <= 10 {
        return BullHandRank {
            hand_type: BullHandType::WuXiaoNiu,
            score: 90_000 + max_rank as u32,
            max_rank,
        };
    }

    // ── 2. 四炸：4 张相同 rank ──
    // 统计每个 rank 出现的次数
    let mut rank_count = [0u8; 14]; // index 0 unused, 1-13
    for c in cards {
        rank_count[c.rank as usize] += 1;
    }
    for r in 1..=13u8 {
        if rank_count[r as usize] == 4 {
            return BullHandRank {
                hand_type: BullHandType::SiZha { rank: r },
                score: 80_000 + r as u32 * 100 + max_rank as u32,
                max_rank,
            };
        }
    }

    // ── 3. 五花牛：5 张全是 J/Q/K ──
    let all_face = cards.iter().all(|c| c.is_face());
    if all_face {
        return BullHandRank {
            hand_type: BullHandType::WuHuaNiu,
            score: 70_000 + max_rank as u32,
            max_rank,
        };
    }

    // ── 4. 普通牛型判定 ──
    // 枚举 C(5,3) = 10 种 3 张组合，找是否有总和为 10 的倍数
    let points: Vec<u8> = cards.iter().map(|c| c.point()).collect();
    let mut best_niu: Option<u8> = None; // 牛几（0 = 牛牛 = 10）

    for i in 0..5 {
        for j in (i + 1)..5 {
            for k in (j + 1)..5 {
                let three_sum = points[i] as u16 + points[j] as u16 + points[k] as u16;
                if three_sum % 10 == 0 {
                    // 有牛！剩余 2 张的点数和的个位
                    let remaining_sum: u16 = (0..5)
                        .filter(|&idx| idx != i && idx != j && idx != k)
                        .map(|idx| points[idx] as u16)
                        .sum();
                    let niu_val = (remaining_sum % 10) as u8;
                    // niu_val = 0 表示牛牛（最强普通牛）
                    let effective = if niu_val == 0 { 10 } else { niu_val };
                    match best_niu {
                        None => best_niu = Some(effective),
                        Some(prev) => {
                            if effective > prev {
                                best_niu = Some(effective);
                            }
                        }
                    }
                }
            }
        }
    }

    match best_niu {
        Some(10) => BullHandRank {
            hand_type: BullHandType::NiuNiu,
            score: 60_000 + max_rank as u32,
            max_rank,
        },
        Some(n) => BullHandRank {
            hand_type: BullHandType::NiuN { n },
            score: 50_000 + n as u32 * 100 + max_rank as u32,
            max_rank,
        },
        None => BullHandRank {
            hand_type: BullHandType::NoNiu,
            score: max_rank as u32,
            max_rank,
        },
    }
}

/// 牌型描述文字
pub fn bull_hand_type_name(ht: &BullHandType) -> &'static str {
    match ht {
        BullHandType::WuXiaoNiu => "Wu Xiao Niu (Five Small)",
        BullHandType::SiZha { .. } => "Si Zha (Four Bomb)",
        BullHandType::WuHuaNiu => "Wu Hua Niu (Five Face)",
        BullHandType::NiuNiu => "Niu Niu (Bull Bull)",
        BullHandType::NiuN { n } => match n {
            9 => "Niu 9",
            8 => "Niu 8",
            7 => "Niu 7",
            6 => "Niu 6",
            5 => "Niu 5",
            4 => "Niu 4",
            3 => "Niu 3",
            2 => "Niu 2",
            1 => "Niu 1",
            _ => "Niu ?",
        },
        BullHandType::NoNiu => "No Niu (No Bull)",
    }
}

/// 根据玩家赢时的牌型确定赔率（含本金）
pub fn bull_payout_multiplier(ht: &BullHandType) -> u128 {
    match ht {
        BullHandType::WuXiaoNiu => 8,
        BullHandType::SiZha { .. } => 7,
        BullHandType::WuHuaNiu => 6,
        BullHandType::NiuNiu => 4,
        BullHandType::NiuN { n } if *n >= 8 => 3,
        BullHandType::NiuN { .. } => 2,
        BullHandType::NoNiu => 2,
    }
}

