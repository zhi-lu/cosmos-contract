use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// 基诺（Keno）数据类型
//
// 规则：
//   - 号码池：1-80
//   - 玩家从中选择 1-10 个号码
//   - 系统随机开出 20 个号码
//   - 根据玩家选择的数量和命中数量决定赔率
//
// 赔率表（含本金，选 N 个命中 M 个）：
//   选1：中1 → 3×
//   选2：中2 → 6×
//   选3：中2 → 2×, 中3 → 16×
//   选4：中2 → 1×(退本), 中3 → 5×, 中4 → 30×
//   选5：中3 → 2×, 中4 → 12×, 中5 → 50×
//   选6：中3 → 1×(退本), 中4 → 5×, 中5 → 30×, 中6 → 100×
//   选7：中3 → 1×(退本), 中4 → 3×, 中5 → 12×, 中6 → 50×, 中7 → 200×
//   选8：中4 → 2×, 中5 → 8×, 中6 → 30×, 中7 → 100×, 中8 → 500×
//   选9：中4 → 1×(退本), 中5 → 4×, 中6 → 15×, 中7 → 50×, 中8 → 200×, 中9 → 1000×
//   选10：中5 → 2×, 中6 → 8×, 中7 → 25×, 中8 → 100×, 中9 → 500×, 中10 → 2000×
//   未达到最低命中数 → 0（输）
// ─────────────────────────────────────────────────────────────

/// 基诺开奖结果
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct KenoResult {
    /// 玩家选择的号码
    pub picks: Vec<u8>,
    /// 系统开出的 20 个号码
    pub drawn: Vec<u8>,
    /// 命中的号码
    pub hits: Vec<u8>,
    /// 命中数量
    pub hit_count: u8,
}

/// 验证玩家选号是否合法
pub fn validate_picks(picks: &[u8]) -> Result<(), &'static str> {
    if picks.is_empty() || picks.len() > 10 {
        return Err("Must pick between 1 and 10 numbers");
    }

    // 检查号码范围和重复
    let mut seen = [false; 81]; // index 0 unused, 1-80
    for &n in picks {
        if n < 1 || n > 80 {
            return Err("Numbers must be between 1 and 80");
        }
        if seen[n as usize] {
            return Err("Duplicate numbers are not allowed");
        }
        seen[n as usize] = true;
    }

    Ok(())
}

/// 计算命中号码
pub fn calculate_hits(picks: &[u8], drawn: &[u8]) -> Vec<u8> {
    picks
        .iter()
        .filter(|p| drawn.contains(p))
        .copied()
        .collect()
}

/// 根据选号数量和命中数量计算赔率倍数（含本金，0 表示未中奖）
pub fn keno_payout_multiplier(pick_count: u8, hit_count: u8) -> u128 {
    match (pick_count, hit_count) {
        // 选 1
        (1, 1) => 3,

        // 选 2
        (2, 2) => 6,

        // 选 3
        (3, 2) => 2,
        (3, 3) => 16,

        // 选 4
        (4, 2) => 1,  // 退还本金
        (4, 3) => 5,
        (4, 4) => 30,

        // 选 5
        (5, 3) => 2,
        (5, 4) => 12,
        (5, 5) => 50,

        // 选 6
        (6, 3) => 1,  // 退还本金
        (6, 4) => 5,
        (6, 5) => 30,
        (6, 6) => 100,

        // 选 7
        (7, 3) => 1,  // 退还本金
        (7, 4) => 3,
        (7, 5) => 12,
        (7, 6) => 50,
        (7, 7) => 200,

        // 选 8
        (8, 4) => 2,
        (8, 5) => 8,
        (8, 6) => 30,
        (8, 7) => 100,
        (8, 8) => 500,

        // 选 9
        (9, 4) => 1,  // 退还本金
        (9, 5) => 4,
        (9, 6) => 15,
        (9, 7) => 50,
        (9, 8) => 200,
        (9, 9) => 1000,

        // 选 10
        (10, 5) => 2,
        (10, 6) => 8,
        (10, 7) => 25,
        (10, 8) => 100,
        (10, 9) => 500,
        (10, 10) => 2000,

        // 其他情况（未达到最低命中数）
        _ => 0,
    }
}

