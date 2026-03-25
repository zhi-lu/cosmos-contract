use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// 三公（San Gong / Three Face Cards）数据类型
//
// 规则：
//   - 使用一副标准 52 张牌，每人发 3 张
//   - 点数计算：A=1, 2-9=面值, 10/J/Q/K=0（公牌）
//   - 取三张牌之和的个位数作为最终点数（0-9）
//   - 特殊牌型（从高到低）：
//     1. 三公 (San Gong)：三张都是公牌（10/J/Q/K），最大牌型
//     2. 混合九 (Mixed Nine)：含公牌且点数为 9
//     3. 普通点数：0-9，9 最大 0 最小
//   - 点数相同时比较最大单牌
//   - 赔率：
//     - 三公赢：3× 返还
//     - 混合九赢：2.5× 返还
//     - 普通赢：2× 返还
//     - 平局：退还本金
// ─────────────────────────────────────────────────────────────

/// 花色（仅用于显示，三公中花色不影响牌力）
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

/// 三公的牌：rank 1=A, 2-10, 11=J, 12=Q, 13=K
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub struct SanGongCard {
    pub rank: u8, // 1..=13
    pub suit: Suit,
}

impl SanGongCard {
    /// 从 0..=51 的牌 ID 构造
    pub fn from_id(id: u8) -> Self {
        let rank = id / 4 + 1; // 0-51 => rank 1-13
        let suit = Suit::from_u8(id % 4);
        SanGongCard { rank, suit }
    }

    /// 该牌的三公点数：A=1, 2-9=面值, 10/J/Q/K=0
    pub fn point_value(&self) -> u8 {
        if self.rank >= 10 {
            0 // 10, J, Q, K 都是公牌，点数为 0
        } else {
            self.rank // A=1, 2=2, ..., 9=9
        }
    }

    /// 是否是公牌（10/J/Q/K）
    pub fn is_face_card(&self) -> bool {
        self.rank >= 10
    }
}

// ─────────────────────────────────────────────────────────────
// 三公手牌评估
// ─────────────────────────────────────────────────────────────

/// 三公牌型分类
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum SanGongHandType {
    /// 三公：三张全是公牌（10/J/Q/K）
    SanGong,
    /// 混合九：含有公牌且点数为 9
    MixedNine,
    /// 普通点数（0-9）
    Normal { points: u8 },
}

/// 三公手牌评估结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SanGongHandRank {
    /// 牌型
    pub hand_type: SanGongHandType,
    /// 用于比较的综合分数（越大越强）
    pub score: u32,
}

/// 评估三张牌的三公牌力
pub fn evaluate_sangong_hand(cards: &[SanGongCard; 3]) -> SanGongHandRank {
    let face_count = cards.iter().filter(|c| c.is_face_card()).count();
    let total_points: u8 = cards.iter().map(|c| c.point_value()).sum::<u8>() % 10;

    // 最大单牌 rank（用于同点比较）
    let max_rank = cards.iter().map(|c| c.rank).max().unwrap_or(0) as u32;

    // 三公：三张都是公牌
    if face_count == 3 {
        return SanGongHandRank {
            hand_type: SanGongHandType::SanGong,
            score: 30_000 + max_rank, // 最高等级
        };
    }

    // 混合九：含有公牌且点数恰好为 9
    if face_count > 0 && total_points == 9 {
        return SanGongHandRank {
            hand_type: SanGongHandType::MixedNine,
            score: 20_000 + max_rank, // 次高等级
        };
    }

    // 普通点数
    SanGongHandRank {
        hand_type: SanGongHandType::Normal { points: total_points },
        score: (total_points as u32) * 100 + max_rank,
    }
}

/// 获取牌型描述文字
pub fn hand_type_name(hand_type: &SanGongHandType) -> &'static str {
    match hand_type {
        SanGongHandType::SanGong => "San Gong (Three Face Cards)",
        SanGongHandType::MixedNine => "Mixed Nine",
        SanGongHandType::Normal { .. } => "Normal Points",
    }
}

/// 根据牌型确定赔率倍数（含本金）
pub fn payout_multiplier(hand_type: &SanGongHandType) -> u128 {
    match hand_type {
        SanGongHandType::SanGong => 3,     // 三公赢 3×
        SanGongHandType::MixedNine => 2,   // 混合九赢 2× (简化，避免小数)
        SanGongHandType::Normal { .. } => 2, // 普通赢 2×
    }
}

