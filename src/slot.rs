use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────
// 游戏模式
// ─────────────────────────────────────────────

/// Basic    = 3 轮 × 1 行（原始玩法）
/// Advanced = 5 轮 × 3 行，支持 5 条赢线
/// Mega     = 6 轮 × 4 行，支持 10 条赢线 + 免费旋转倍率 + Jackpot 彩金
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotMode {
    Basic,
    Advanced,
    Mega,
}

// ─────────────────────────────────────────────
// 符号定义
// ─────────────────────────────────────────────

/// Wild  可替代任意普通符号（不能替代 Scatter）
/// Scatter 出现 3+ 个触发散布奖励（倍率加成）
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Symbol {
    Apple,
    Orange,
    Cherry,
    Lemon,
    Bell,
    Seven,
    Bar,
    Wild,
    Scatter,
}

impl Symbol {
    /// 根据随机数映射符号（含 Wild / Scatter 低概率）
    /// 输入已经是 0..=99 的随机值
    pub fn from_u8(value: u32) -> Self {
        match value % 100 {
            0..=14 => Symbol::Apple,    // 15%
            15..=26 => Symbol::Orange,  // 12%
            27..=37 => Symbol::Cherry,  // 11%
            38..=48 => Symbol::Lemon,   // 11%
            49..=58 => Symbol::Bell,    // 10%
            59..=68 => Symbol::Seven,   // 10%
            69..=76 => Symbol::Bar,     //  8%
            77..=88 => Symbol::Wild,    // 12%  Wild 出现频率较高方便触发
            _       => Symbol::Scatter, //  11%
        }
    }

    /// 三连或以上的基础倍率（Basic 模式 / Advanced 中每条赢线）
    pub fn payout_multiplier(&self) -> u64 {
        match self {
            Symbol::Apple   => 2,
            Symbol::Orange  => 3,
            Symbol::Cherry  => 4,
            Symbol::Lemon   => 4,
            Symbol::Bell    => 5,
            Symbol::Seven   => 8,
            Symbol::Bar     => 16,
            Symbol::Wild    => 10, // Wild 自成一线时给高倍率
            Symbol::Scatter => 0,  // Scatter 不参与连线
        }
    }

    /// 是否为 Wild 符号
    pub fn is_wild(&self) -> bool {
        matches!(self, Symbol::Wild)
    }

    /// 是否为 Scatter 符号
    pub fn is_scatter(&self) -> bool {
        matches!(self, Symbol::Scatter)
    }
}

// ─────────────────────────────────────────────
// 赢线定义（仅 Advanced 模式使用）
//
// 5 列 × 3 行的格子，每个元素为 (col, row)
// row: 0=上, 1=中, 2=下
// ─────────────────────────────────────────────

/// 返回 Advanced 模式所有赢线的列-行索引列表
/// 每条赢线包含 5 个位置 (col_index, row_index)
pub fn paylines() -> Vec<Vec<(usize, usize)>> {
    vec![
        // 赢线 1: 中间横线
        vec![(0,1),(1,1),(2,1),(3,1),(4,1)],
        // 赢线 2: 上横线
        vec![(0,0),(1,0),(2,0),(3,0),(4,0)],
        // 赢线 3: 下横线
        vec![(0,2),(1,2),(2,2),(3,2),(4,2)],
        // 赢线 4: 向下对角线
        vec![(0,0),(1,1),(2,2),(3,1),(4,0)],
        // 赢线 5: 向上对角线
        vec![(0,2),(1,1),(2,0),(3,1),(4,2)],
    ]
}

// ─────────────────────────────────────────────
// Basic 模式结算（3 轮 × 1 行）
// ─────────────────────────────────────────────

/// 结算结果
pub struct SlotPayout {
    pub multiplier: u64,
    pub description: String,
}

/// Basic 模式：3 个符号，Wild 可替代普通符号
pub fn evaluate_basic(s1: &Symbol, s2: &Symbol, s3: &Symbol) -> SlotPayout {
    // 将 Wild 视为被替代符号进行匹配
    let effective = resolve_wilds_3(&[s1, s2, s3]);

    if effective[0] == effective[1] && effective[1] == effective[2] {
        let mult = effective[0].payout_multiplier();
        SlotPayout {
            multiplier: mult,
            description: format!("3x {:?}", effective[0]),
        }
    } else if effective[0] == effective[1] || effective[0] == effective[2] || effective[1] == effective[2] {
        let matched = if effective[0] == effective[1] || effective[0] == effective[2] {
            &effective[0]
        } else {
            &effective[1]
        };
        let mult = matched.payout_multiplier() / 2;
        SlotPayout {
            multiplier: mult,
            description: format!("2x {:?}", matched),
        }
    } else {
        SlotPayout {
            multiplier: 0,
            description: "no_match".to_string(),
        }
    }
}

/// 将 3 个符号中的 Wild 替换为最优的普通符号
fn resolve_wilds_3<'a>(symbols: &[&'a Symbol; 3]) -> [&'a Symbol; 3] {
    // 找到非 Wild 非 Scatter 中出现最多的符号
    let best = best_non_wild(symbols.iter().copied());
    symbols.map(|s| if s.is_wild() { best.unwrap_or(s) } else { s })
}

// ─────────────────────────────────────────────
// Advanced 模式结算（5 列 × 3 行）
// ─────────────────────────────────────────────

/// 计算 Advanced 模式总倍率
/// grid[col][row]
pub fn evaluate_advanced(grid: &[[Symbol; 3]; 5]) -> (u64, Vec<String>) {
    let mut total_multiplier: u64 = 0;
    let mut win_descriptions: Vec<String> = Vec::new();

    // 1. 计算每条赢线
    for (line_idx, line) in paylines().iter().enumerate() {
        let syms: Vec<&Symbol> = line.iter().map(|(c, r)| &grid[*c][*r]).collect();
        let (mult, desc) = evaluate_payline(&syms);
        if mult > 0 {
            total_multiplier += mult;
            win_descriptions.push(format!("line{}:{}", line_idx + 1, desc));
        }
    }

    // 2. Scatter 奖励：统计整个 grid 中 Scatter 数量
    let scatter_count = grid
        .iter()
        .flat_map(|col| col.iter())
        .filter(|s| s.is_scatter())
        .count();

    let scatter_bonus = scatter_bonus_multiplier(scatter_count);
    if scatter_bonus > 0 {
        total_multiplier += scatter_bonus;
        win_descriptions.push(format!("scatter_{}x_bonus:{}", scatter_count, scatter_bonus));
    }

    (total_multiplier, win_descriptions)
}

/// 对一条赢线（5 个符号）计算连线倍率
/// 从左到右找最长连续匹配（Wild 可替代）
fn evaluate_payline(syms: &[&Symbol]) -> (u64, String) {
    // 先取第一个有效符号（非 Scatter，Wild 暂时留着）
    let anchor = first_non_scatter_non_wild(syms);
    if anchor.is_none() {
        return (0, "no_anchor".to_string());
    }
    let anchor = anchor.unwrap();

    let mut count = 0usize;
    for s in syms {
        if s.is_wild() || *s == anchor {
            count += 1;
        } else {
            break; // 必须从最左边开始连续
        }
    }

    let mult = match count {
        5 => anchor.payout_multiplier() * 5,
        4 => anchor.payout_multiplier() * 3,
        3 => anchor.payout_multiplier(),
        _ => 0,
    };

    (mult, format!("{:?}x{}", anchor, count))
}

/// 找第一个非 Scatter 非 Wild 符号
fn first_non_scatter_non_wild<'a>(syms: &[&'a Symbol]) -> Option<&'a Symbol> {
    syms.iter().find(|s| !s.is_wild() && !s.is_scatter()).copied()
}

/// Scatter 数量 → 额外奖励倍率
fn scatter_bonus_multiplier(count: usize) -> u64 {
    match count {
        3 => 5,
        4 => 15,
        5..=usize::MAX => 50,
        _ => 0,
    }
}

/// 从迭代器中找出最优（payout 最高）的非 Wild、非 Scatter 符号
fn best_non_wild<'a>(syms: impl Iterator<Item = &'a Symbol>) -> Option<&'a Symbol> {
    syms.filter(|s| !s.is_wild() && !s.is_scatter())
        .max_by_key(|s| s.payout_multiplier())
}

// ─────────────────────────────────────────────
// Mega 模式结算（6 列 × 4 行，10 条赢线）
// ─────────────────────────────────────────────

/// 返回 Mega 模式所有赢线（10 条），每条包含 6 个位置 (col, row)
pub fn mega_paylines() -> Vec<Vec<(usize, usize)>> {
    vec![
        // 横线：4 行各一条
        vec![(0,0),(1,0),(2,0),(3,0),(4,0),(5,0)],
        vec![(0,1),(1,1),(2,1),(3,1),(4,1),(5,1)],
        vec![(0,2),(1,2),(2,2),(3,2),(4,2),(5,2)],
        vec![(0,3),(1,3),(2,3),(3,3),(4,3),(5,3)],
        // 对角线
        vec![(0,0),(1,1),(2,2),(3,3),(4,2),(5,1)], // ∨ 型
        vec![(0,3),(1,2),(2,1),(3,0),(4,1),(5,2)], // ∧ 型
        // 阶梯线
        vec![(0,0),(1,0),(2,1),(3,1),(4,2),(5,2)], // 上到下阶梯
        vec![(0,3),(1,3),(2,2),(3,2),(4,1),(5,1)], // 下到上阶梯
        // Z/S 型
        vec![(0,0),(1,1),(2,1),(3,2),(4,2),(5,3)],
        vec![(0,3),(1,2),(2,2),(3,1),(4,1),(5,0)],
    ]
}

/// Mega 模式结果
pub struct MegaSlotResult {
    pub total_multiplier: u64,
    pub descriptions: Vec<String>,
    /// 是否触发免费旋转（Scatter ≥ 4 触发）
    pub free_spin_triggered: bool,
    /// 免费旋转额外倍率（叠加在总倍率上）
    pub free_spin_multiplier: u64,
    /// 是否命中 Jackpot（全格 Wild）
    pub jackpot: bool,
}

/// 计算 Mega 模式总倍率
/// grid[col][row]，6 列 × 4 行
pub fn evaluate_mega(grid: &[[Symbol; 4]; 6]) -> MegaSlotResult {
    let mut total_multiplier: u64 = 0;
    let mut descriptions: Vec<String> = Vec::new();

    // 1. 计算每条赢线
    for (line_idx, line) in mega_paylines().iter().enumerate() {
        let syms: Vec<&Symbol> = line.iter().map(|(c, r)| &grid[*c][*r]).collect();
        let (mult, desc) = evaluate_mega_payline(&syms);
        if mult > 0 {
            total_multiplier += mult;
            descriptions.push(format!("line{}:{}", line_idx + 1, desc));
        }
    }

    // 2. Scatter 奖励
    let scatter_count = grid
        .iter()
        .flat_map(|col| col.iter())
        .filter(|s| s.is_scatter())
        .count();

    let scatter_bonus = mega_scatter_bonus(scatter_count);
    if scatter_bonus > 0 {
        total_multiplier += scatter_bonus;
        descriptions.push(format!("scatter_{}x_bonus:{}", scatter_count, scatter_bonus));
    }

    // 3. 免费旋转（Scatter ≥ 4 触发）
    let free_spin_triggered = scatter_count >= 4;
    let free_spin_multiplier = if free_spin_triggered {
        // 免费旋转额外倍率：scatter 数量越多倍率越高
        match scatter_count {
            4 => 2,
            5 => 5,
            6..=usize::MAX => 10,
            _ => 0,
        }
    } else {
        0
    };

    if free_spin_multiplier > 0 && total_multiplier > 0 {
        // 将已有倍率再乘以免费旋转倍率
        total_multiplier *= free_spin_multiplier;
        descriptions.push(format!("free_spin_x{}", free_spin_multiplier));
    }

    // 4. Jackpot：全部 24 格都是 Wild
    let wild_count = grid
        .iter()
        .flat_map(|col| col.iter())
        .filter(|s| s.is_wild())
        .count();
    let jackpot = wild_count == 24;
    if jackpot {
        total_multiplier = total_multiplier.max(10_000); // 保底 Jackpot 倍率
        descriptions.push("JACKPOT".to_string());
    }

    MegaSlotResult {
        total_multiplier,
        descriptions,
        free_spin_triggered,
        free_spin_multiplier,
        jackpot,
    }
}

/// 对 Mega 赢线（6 个符号）计算连线倍率
/// 从左到右：3/4/5/6 连续相同（Wild 可替代）
fn evaluate_mega_payline(syms: &[&Symbol]) -> (u64, String) {
    let anchor = first_non_scatter_non_wild(syms);
    if anchor.is_none() {
        return (0, "no_anchor".to_string());
    }
    let anchor = anchor.unwrap();

    let mut count = 0usize;
    for s in syms {
        if s.is_wild() || *s == anchor {
            count += 1;
        } else {
            break;
        }
    }

    let mult = match count {
        6 => anchor.payout_multiplier() * 10,
        5 => anchor.payout_multiplier() * 5,
        4 => anchor.payout_multiplier() * 3,
        3 => anchor.payout_multiplier(),
        _ => 0,
    };

    (mult, format!("{:?}x{}", anchor, count))
}

/// Mega 模式 Scatter 奖励
fn mega_scatter_bonus(count: usize) -> u64 {
    match count {
        3 => 8,
        4 => 25,
        5 => 75,
        6..=usize::MAX => 200,
        _ => 0,
    }
}

