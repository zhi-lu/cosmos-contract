use cosmwasm_std::{Env, MessageInfo};
use sha2::{Digest, Sha256};

/// 生成一个随机数
///
/// 使用盐来生成一个随机数，并返回一个1到100之间的整数
pub fn generate_random_number(info: &MessageInfo, env: &Env, salt: &[u8]) -> u32 {
    let mut hasher = Sha256::new();
    // 用户地址
    hasher.update(&info.sender.as_bytes());
    // 区块高度
    hasher.update(&env.block.height.to_be_bytes());
    // 区块时间
    hasher.update(&env.block.time.seconds().to_be_bytes());
    if let Some(tx) = &env.transaction {
        // 交易索引
        hasher.update(tx.index.to_be_bytes());
    }
    // 加盐
    hasher.update(salt);
    let hash = hasher.finalize();
    (u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]) % 100) + 1
}

/// 计算 blackjack 的 total 值
///
/// 计算 blackjack 的总点数，自动处理 Ace（A）的 1 or 11 值
pub fn calculate_blackjack_total(cards: &[u32]) -> u32 {
    let mut total = 0;
    let mut ace_count = 0;

    for &card in cards {
        if card == 1 {
            ace_count += 1;
            // Ace 的值也为 11
            total += 11;
        } else {
            total += card;
        }
    }

    // 如果爆了就把 Ace 从 11 调整为 1
    while total > 21 && ace_count > 0 {
        total -= 10;
        ace_count -= 1;
    }

    total
}
