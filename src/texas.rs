use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::Uint128;

// ─────────────────────────────────────────────────────────────
// 德州扑克（Texas Hold'em）数据类型
// 与奥马哈共享 Card/Suit 结构，但独立定义以保持模块解耦
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

/// 牌：点数 2-14（11=J,12=Q,13=K,14=A）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub struct Card {
    pub rank: u8,
    pub suit: Suit,
}

impl Card {
    /// 从 0..=51 的牌 ID 构造
    pub fn from_id(id: u8) -> Self {
        let rank = id / 4 + 2;
        let suit = Suit::from_u8(id % 4);
        Card { rank, suit }
    }
}

// ─────────────────────────────────────────────────────────────
// 游戏阶段
// ─────────────────────────────────────────────────────────────

/// 德州扑克游戏阶段
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum TexasStage {
    PreFlop,  // 发手牌后（玩家 2 张，庄家 2 张）
    Flop,     // 翻牌（3 张公共牌）
    Turn,     // 转牌（第 4 张公共牌）
    River,    // 河牌（第 5 张公共牌）
    Showdown, // 摊牌结算
}

// ─────────────────────────────────────────────────────────────
// 玩家动作
// ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TexasAction {
    /// 开始新游戏，附带初始底注（盲注）
    Start,
    /// 加注：附带 funds 中追加注额
    Raise { amount: u128 },
    /// 跟注：补齐当前最高注
    Call,
    /// 过牌（Check）：当前差额为 0 时无需付款，直接推进阶段
    Check,
    /// 弃牌：放弃本局，损失已下注金额
    Fold,
    /// 全押（All-In）：将指定金额全部押入
    AllIn { amount: u128 },
    /// 摊牌结算
    Showdown,
}

// ─────────────────────────────────────────────────────────────
// 链上游戏状态
// ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TexasState {
    /// 玩家手牌（2 张）
    pub player_hand: Vec<Card>,
    /// 庄家手牌（2 张，Showdown 前隐藏）
    pub dealer_hand: Vec<Card>,
    /// 公共牌（最多 5 张，按阶段揭示）
    pub community_cards: Vec<Card>,
    /// 玩家已下注总额（累计）
    pub player_total_bet: Uint128,
    /// 当前局的最高注额（跟注线）
    pub current_call_amount: Uint128,
    /// 当前游戏阶段
    pub stage: TexasStage,
    /// 游戏是否结束
    pub finished: bool,
    /// 是否处于全押状态
    pub all_in: bool,
    /// 洗牌后的 52 张牌 ID 序列
    pub deck: Vec<u8>,
}

// ─────────────────────────────────────────────────────────────
// 查询响应
// ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TexasStateResponse {
    pub player_hand: Vec<Card>,
    /// Showdown 前隐藏庄家手牌（返回空列表）
    pub dealer_hand: Vec<Card>,
    pub community_cards: Vec<Card>,
    pub player_total_bet: Uint128,
    pub current_call_amount: Uint128,
    pub stage: TexasStage,
    pub finished: bool,
    pub all_in: bool,
}

// ─────────────────────────────────────────────────────────────
// 手牌强度评估（与奥马哈共用同一套评分逻辑）
// 德州规则：用全部 2 张手牌 + 5 张公共牌中任意组合出最佳 5 张
// ─────────────────────────────────────────────────────────────

/// 从 7 张牌（2 手牌 + 5 公共牌）中选出最佳 5 张组合，返回最高评分
pub fn best_texas_hand_rank(hand: &[Card], community: &[Card]) -> u32 {
    let mut all_cards: Vec<Card> = hand.to_vec();
    all_cards.extend_from_slice(community);
    let n = all_cards.len();
    let mut best = 0u32;

    // C(n, 5) 枚举所有 5 张组合
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                for l in (k + 1)..n {
                    for m in (l + 1)..n {
                        let five = [
                            all_cards[i],
                            all_cards[j],
                            all_cards[k],
                            all_cards[l],
                            all_cards[m],
                        ];
                        let rank = evaluate_5card_rank(&five);
                        if rank > best {
                            best = rank;
                        }
                    }
                }
            }
        }
    }
    best
}

/// 对 5 张牌进行手牌强度评估（同花顺 > 四条 > 葫芦 > 同花 > 顺子 > 三条 > 两对 > 一对 > 高牌）
pub fn evaluate_5card_rank(cards: &[Card; 5]) -> u32 {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_unstable_by(|a, b| b.cmp(a));

    let is_flush = cards[0].suit == cards[1].suit
        && cards[1].suit == cards[2].suit
        && cards[2].suit == cards[3].suit
        && cards[3].suit == cards[4].suit;

    let is_straight = {
        let mut s = ranks.clone();
        if s == vec![14, 5, 4, 3, 2] {
            s = vec![5, 4, 3, 2, 1];
        }
        s[0] - s[4] == 4
            && s[0] != s[1]
            && s[1] != s[2]
            && s[2] != s[3]
            && s[3] != s[4]
    };

    // 统计频次
    let mut freq: Vec<(u8, u8)> = Vec::new();
    for &r in &ranks {
        if let Some(entry) = freq.iter_mut().find(|e| e.0 == r) {
            entry.1 += 1;
        } else {
            freq.push((r, 1));
        }
    }
    freq.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

    let counts: Vec<u8> = freq.iter().map(|e| e.1).collect();
    let top_rank = freq[0].0 as u32;

    if is_flush && is_straight {
        return 8_000_000 + ranks[0] as u32;
    }
    if counts[0] == 4 {
        return 7_000_000 + top_rank * 100 + freq[1].0 as u32;
    }
    if counts[0] == 3 && counts.get(1) == Some(&2) {
        return 6_000_000 + top_rank * 100 + freq[1].0 as u32;
    }
    if is_flush {
        return 5_000_000
            + ranks[0] as u32 * 10000
            + ranks[1] as u32 * 1000
            + ranks[2] as u32 * 100
            + ranks[3] as u32 * 10
            + ranks[4] as u32;
    }
    if is_straight {
        return 4_000_000 + ranks[0] as u32;
    }
    if counts[0] == 3 {
        return 3_000_000
            + top_rank * 10000
            + freq.get(1).map(|e| e.0 as u32).unwrap_or(0) * 100
            + freq.get(2).map(|e| e.0 as u32).unwrap_or(0);
    }
    if counts[0] == 2 && counts.get(1) == Some(&2) {
        let p1 = top_rank;
        let p2 = freq[1].0 as u32;
        let kicker = freq.get(2).map(|e| e.0 as u32).unwrap_or(0);
        return 2_000_000 + p1 * 10000 + p2 * 100 + kicker;
    }
    if counts[0] == 2 {
        return 1_000_000
            + top_rank * 100000
            + freq.get(1).map(|e| e.0 as u32).unwrap_or(0) * 1000
            + freq.get(2).map(|e| e.0 as u32).unwrap_or(0) * 100
            + freq.get(3).map(|e| e.0 as u32).unwrap_or(0);
    }
    // 高牌
    ranks[0] as u32 * 10000
        + ranks[1] as u32 * 1000
        + ranks[2] as u32 * 100
        + ranks[3] as u32 * 10
        + ranks[4] as u32
}

/// 获取手牌强度描述文字
pub fn hand_rank_name(rank: u32) -> &'static str {
    if rank >= 8_000_000 {
        "Straight Flush"
    } else if rank >= 7_000_000 {
        "Four of a Kind"
    } else if rank >= 6_000_000 {
        "Full House"
    } else if rank >= 5_000_000 {
        "Flush"
    } else if rank >= 4_000_000 {
        "Straight"
    } else if rank >= 3_000_000 {
        "Three of a Kind"
    } else if rank >= 2_000_000 {
        "Two Pair"
    } else if rank >= 1_000_000 {
        "One Pair"
    } else {
        "High Card"
    }
}

