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
