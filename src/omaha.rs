use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::Uint128;

// ─────────────────────────────────────────────
// 奥马哈扑克（Omaha Hold'em）数据类型
// ─────────────────────────────────────────────

/// 花色
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub enum Suit {
    Spades,   // 黑桃
    Hearts,   // 红心
    Diamonds, // 方块
    Clubs,    // 梅花
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

/// 点数：2-14，其中 11=J，12=Q，13=K，14=A
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Copy)]
pub struct Card {
    pub rank: u8, // 2..=14
    pub suit: Suit,
}

impl Card {
    /// 从 0..=51 的牌 ID 构造
    pub fn from_id(id: u8) -> Self {
        let rank = id / 4 + 2; // 0-51 => rank 2-14
        let suit = Suit::from_u8(id % 4);
        Card { rank, suit }
    }
}

// ─────────────────────────────────────────────
// 游戏阶段
// ─────────────────────────────────────────────

/// 奥马哈游戏阶段
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum OmahaStage {
    PreFlop,  // 发手牌后
    Flop,     // 翻牌后（3 张公共牌）
    Turn,     // 转牌后（第 4 张）
    River,    // 河牌后（第 5 张）
    Showdown, // 摊牌/结算
}

// ─────────────────────────────────────────────
// 玩家动作
// ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OmahaAction {
    /// 开始新游戏，附带初始底注
    Start,
    /// 加注（追加金额）
    Raise { amount: u128 },
    /// 跟注（按当前最高注额跟进）
    Call,
    /// 弃牌（放弃本局，损失已下注金额）
    Fold,
    /// 摊牌结算（进入 Showdown）
    Showdown,
}

// ─────────────────────────────────────────────
// 游戏状态（存储在链上）
// ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OmahaState {
    /// 玩家手牌（4 张）
    pub player_hand: Vec<Card>,
    /// 庄家/合约手牌（4 张，不公开直到 Showdown）
    pub dealer_hand: Vec<Card>,
    /// 公共牌（最多 5 张，按阶段逐步揭示）
    pub community_cards: Vec<Card>,
    /// 玩家已下注总额
    pub player_total_bet: Uint128,
    /// 合约当前最高注额（庄家/对手侧注）
    pub current_call_amount: Uint128,
    /// 当前游戏阶段
    pub stage: OmahaStage,
    /// 游戏是否结束
    pub finished: bool,
    /// 初始 52 张牌的洗牌顺序（按 card_id）存储以便后续阶段续发
    pub deck: Vec<u8>,
    /// 当前发到第几张牌（索引）
    pub deck_pos: u8,
}

// ─────────────────────────────────────────────
// 查询响应
// ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OmahaStateResponse {
    pub player_hand: Vec<Card>,
    /// 未结束时隐藏庄家手牌
    pub dealer_hand: Vec<Card>,
    pub community_cards: Vec<Card>,
    pub player_total_bet: Uint128,
    pub current_call_amount: Uint128,
    pub stage: OmahaStage,
    pub finished: bool,
}

// ─────────────────────────────────────────────
// 手牌强度评估
// ─────────────────────────────────────────────

/// 奥马哈规则：必须恰好用 2 张手牌 + 3 张公共牌组成最佳 5 牌组合。
/// 此处简化：枚举所有 C(4,2) × C(5,3) = 6×10 = 60 种组合，取最高评分。
pub fn best_omaha_hand_rank(hand: &[Card], community: &[Card]) -> u32 {
    if community.len() < 3 {
        return 0;
    }
    let mut best = 0u32;
    // 枚举手牌中选 2 张
    for i in 0..hand.len() {
        for j in (i + 1)..hand.len() {
            // 枚举公共牌中选 3 张
            for a in 0..community.len() {
                for b in (a + 1)..community.len() {
                    for c in (b + 1)..community.len() {
                        let five = [
                            hand[i],
                            hand[j],
                            community[a],
                            community[b],
                            community[c],
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

/// 对 5 张牌进行手牌强度评估，返回数值越大表示越强。
/// 从高到低：
///   同花顺 > 四条 > 葫芦 > 同花 > 顺子 > 三条 > 两对 > 一对 > 高牌
pub fn evaluate_5card_rank(cards: &[Card; 5]) -> u32 {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_unstable_by(|a, b| b.cmp(a)); // 降序

    let is_flush = cards.windows(1).all(|_| true)
        && cards[0].suit == cards[1].suit
        && cards[1].suit == cards[2].suit
        && cards[2].suit == cards[3].suit
        && cards[3].suit == cards[4].suit;

    // 顺子判断（A 可作 1 用于 A-2-3-4-5）
    let is_straight = {
        let mut s = ranks.clone();
        // 处理 A-2-3-4-5 小顺子
        if s == vec![14, 5, 4, 3, 2] {
            s = vec![5, 4, 3, 2, 1];
        }
        s[0] - s[4] == 4
            && s[0] != s[1]
            && s[1] != s[2]
            && s[2] != s[3]
            && s[3] != s[4]
    };

    // 统计点数频次
    let mut freq: Vec<(u8, u8)> = Vec::new(); // (rank, count)
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

    // 同花顺
    if is_flush && is_straight {
        return 8_000_000 + ranks[0] as u32;
    }
    // 四条
    if counts[0] == 4 {
        return 7_000_000 + top_rank * 100 + freq[1].0 as u32;
    }
    // 葫芦
    if counts[0] == 3 && counts[1] == 2 {
        return 6_000_000 + top_rank * 100 + freq[1].0 as u32;
    }
    // 同花
    if is_flush {
        return 5_000_000
            + ranks[0] as u32 * 10000
            + ranks[1] as u32 * 1000
            + ranks[2] as u32 * 100
            + ranks[3] as u32 * 10
            + ranks[4] as u32;
    }
    // 顺子
    if is_straight {
        return 4_000_000 + ranks[0] as u32;
    }
    // 三条
    if counts[0] == 3 {
        return 3_000_000 + top_rank * 10000 + freq[1].0 as u32 * 100 + freq[2].0 as u32;
    }
    // 两对
    if counts[0] == 2 && counts[1] == 2 {
        let p1 = top_rank;
        let p2 = freq[1].0 as u32;
        let kicker = freq[2].0 as u32;
        return 2_000_000 + p1 * 10000 + p2 * 100 + kicker;
    }
    // 一对
    if counts[0] == 2 {
        return 1_000_000
            + top_rank * 100000
            + freq[1].0 as u32 * 1000
            + freq[2].0 as u32 * 100
            + freq[3].0 as u32;
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

