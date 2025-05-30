mod msg;
mod slot;
mod state;
mod utils;

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::slot::Symbol;
use crate::state::{LockedAmountResponse, State, STATE};
use cosmwasm_std::{
    entry_point, to_json_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    // 需要的最少锁仓金额
    let required_minimum_lock_coin_amount = 10_000_000_000;

    let received = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount)
        .unwrap_or_else(Uint128::zero);

    if received.u128() < required_minimum_lock_coin_amount {
        return Err(StdError::generic_err(format!(
            "Received amount {} < required minimum lock amount {}",
            received, required_minimum_lock_coin_amount
        )));
    }

    // 存储初始状态
    let state = State {
        owner: info.sender.clone(),
        locked_amount: received.u128(),
    };

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "init")
        .add_attribute("owner", info.sender)
        .add_attribute("locked_amount", received.to_string()))
}

/// 处理执行逻辑
/// 目前支持比大小的游戏逻辑未来会继续扩展
/// 包括合约管理员提取锁仓代币的逻辑
#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Play {} => play_game(deps, env, info),
        ExecuteMsg::PlaySlot {} => play_slot(deps, env, info),
        ExecuteMsg::Withdraw { amount } => withdraw_funds(deps, info, amount),
    }
}

/// 处理查询逻辑
/// 查询当前合约还有多少锁仓代币
#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLockedAmount {} => {
            let state = STATE.load(deps.storage)?;
            let resp = LockedAmountResponse {
                locked_amount: Uint128::from(state.locked_amount),
            };
            to_json_binary(&resp)
        }
    }
}

/// 比大小游戏
///
/// 用户和合约进行比大小游戏, 用户生成的数字大于合约生成的数字，则用户获胜，获得下注金额 ×2 的奖励.
fn play_game(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    // 检查用户是否发送了正确金额
    let sent_amount = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    if sent_amount < 100_000 || sent_amount > 10_000_000 {
        return Err(StdError::generic_err(
            "Bet must be between 100,000 and 10,000,000 uatom",
        ));
    }

    // 修改 State 的 locked_amount 值
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += sent_amount;

    // 生成用户 1～100 的随机数
    let user_rand = utils::generate_random_number(&info, &env, b"user");
    // 生成的合约 1～100 的随机数
    let contract_rand = utils::generate_random_number(&info, &env, b"contract");

    // 比较结果
    let mut response = Response::new();
    let mut user_win = "lost";
    if user_rand > contract_rand {
        // 用户赢：发送奖励（下注金额 ×2）
        let payout = Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(sent_amount * 2),
        };
        response = response.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![payout],
        });
        state.locked_amount -= sent_amount * 2;
        user_win = "win"
    } else if user_rand == contract_rand {
        // 平局：退还下注
        let refund = Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(sent_amount),
        };
        response = response.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![refund],
        });
        state.locked_amount -= sent_amount;
        user_win = "tie"
    }

    // 更新锁仓金额
    // 用户输：无需操作（资金留在合约）
    STATE.save(deps.storage, &state)?;

    Ok(response
        .add_attribute(
            "result",
            format!("user:{}, contract:{}", user_rand, contract_rand),
        )
        .add_attribute("user_win", user_win))
}

/// 老虎机游戏
///
/// slot 的游戏, 如果用户中了 3 个相同符号，则用户中了该符号的全部奖励, 如果中了 2 个相同符号，则用户中了该符号的 1/2 的奖励, 否则啥奖励都没有
fn play_slot(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    // 检查用户是否发送了正确金额
    let sent_amount = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    if sent_amount < 100_000 || sent_amount > 10_000_000 {
        return Err(StdError::generic_err(
            "Bet must be between 100,000 and 10,000,000 uatom",
        ));
    }

    // 修改 State 的 locked_amount 值
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += sent_amount;

    // 创建 3 个随机数
    let rand1 = utils::generate_random_number(&info, &env, b"slot1");
    let rand2 = utils::generate_random_number(&info, &env, b"slot2");
    let rand3 = utils::generate_random_number(&info, &env, b"slot3");

    // 生成 3 个老虎机符号
    let symbol1 = Symbol::from_u8(rand1);
    let symbol2 = Symbol::from_u8(rand2);
    let symbol3 = Symbol::from_u8(rand3);

    let mut payout_multiplier = 0;

    if symbol1 == symbol2 && symbol2 == symbol3 {
        // 三个相同：全额奖励
        payout_multiplier = symbol1.payout_multiplier();
    } else if symbol1 == symbol2 || symbol1 == symbol3 || symbol2 == symbol3 {
        // 任意两个相同：半额奖励
        let matched = if symbol1 == symbol2 || symbol1 == symbol3 {
            &symbol1
        } else {
            &symbol2
        };
        payout_multiplier = matched.payout_multiplier() / 2;
    }

    let mut response = Response::new()
        .add_attribute("slot1", format!("{:?}", symbol1))
        .add_attribute("slot2", format!("{:?}", symbol2))
        .add_attribute("slot3", format!("{:?}", symbol3))
        .add_attribute("bet_amount", sent_amount.to_string());

    if payout_multiplier > 0 {
        // 获取奖励
        let payout_amount = sent_amount * payout_multiplier as u128;
        let payout = Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(payout_amount),
        };
        // 发送奖励
        response = response.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![payout],
        });
        // 更新锁仓金额
        state.locked_amount -= payout_amount;
        response = response.add_attribute("payout_multiplier", payout_multiplier.to_string());
    } else {
        response = response.add_attribute("result", "lost");
    }

    // 保存状态
    STATE.save(deps.storage, &state)?;

    Ok(response)
}

// 处理提款逻辑（仅限所有者）
fn withdraw_funds(deps: DepsMut, info: MessageInfo, amount: u128) -> StdResult<Response> {
    // 提取的钱不少于 0 uatom
    if amount == 0 {
        return Err(StdError::generic_err("Invalid amount"));
    }

    let mut state = STATE.load(deps.storage)?;

    // 检查调用者是否为所有者
    if info.sender != state.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // 检查可提款余额
    if amount > state.locked_amount {
        return Err(StdError::generic_err("Insufficient locked funds"));
    }

    // 更新锁仓金额
    state.locked_amount -= amount;
    STATE.save(deps.storage, &state)?;

    // 发送代币
    let payout = Coin {
        denom: "uatom".to_string(),
        amount: Uint128::from(amount),
    };

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: state.owner.to_string(),
            amount: vec![payout],
        })
        .add_attribute("action", "withdraw")
        .add_attribute("amount", amount.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_json};

    #[test]
    pub fn test_init() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
        assert_eq!("init", res.attributes[0].value);
        assert_eq!("creator", res.attributes[1].value);
        assert_eq!("10000000000", res.attributes[2].value)
    }

    #[test]
    pub fn test_query() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetLockedAmount {}).unwrap();
        let value: LockedAmountResponse = from_json(&res).unwrap();
        assert_eq!(
            value,
            LockedAmountResponse {
                locked_amount: Uint128::from(10000000000u128)
            }
        );
    }

    #[test]
    pub fn test_play_game() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let res = play_game(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(100_000, "uatom")),
        )
        .unwrap();
        println!("{:?}", res);
        let user_win = &res.attributes[1].value;
        if "win" == user_win {
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: "user".to_string(),
                    amount: coins(200_000, "uatom")
                }
                .into()
            );
        } else if "tie" == user_win {
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: "user".to_string(),
                    amount: coins(100_000, "uatom")
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    pub fn test_play_slot() {
        let mut deps = mock_dependencies();

        // 初始化合约
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 模拟一次用户下注并执行 slot 游戏
        let user_info = mock_info("user", &coins(100_000, "uatom"));
        let env = mock_env();
        let res = play_slot(deps.as_mut(), env.clone(), user_info.clone()).unwrap();
        
        println!("{:?}", res);
        
        // 检查返回的事件属性
        let attrs = res.attributes;

        let slot1 = attrs
            .iter()
            .find(|a| a.key == "slot1")
            .expect("slot1 missing");
        let slot2 = attrs
            .iter()
            .find(|a| a.key == "slot2")
            .expect("slot2 missing");
        let slot3 = attrs
            .iter()
            .find(|a| a.key == "slot3")
            .expect("slot3 missing");

        // 输出调试信息（可选）
        println!(
            "slot1: {}, slot2: {}, slot3: {}",
            slot1.value, slot2.value, slot3.value
        );
        
        // 如果用户赢了或平局，应该包含 payout_multiplier，否则包含 result = lost
        let payout_attr = attrs.iter().find(|a| a.key == "payout_multiplier");
        let result_attr = attrs.iter().find(|a| a.key == "result");

        // 确保包含下注金额
        let bet_attr = attrs.iter().find(|a| a.key == "bet_amount").expect("bet_amount missing");
        assert_eq!(bet_attr.value, "100000");
        
        // 至少应该有一个
        assert!(
            payout_attr.is_some() || result_attr.is_some(),
            "expected either payout_multiplier or result"
        );

        // 可选：打印测试输出帮助调试
        if let Some(attr) = payout_attr {
            println!("User won with multiplier: {}", attr.value);
        } else if let Some(attr) = result_attr {
            println!("User lost: {}", attr.value);
        }
    }

    #[test]
    pub fn test_withdraw_funds() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        let info_clone = info.clone();
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let res = withdraw_funds(deps.as_mut(), info_clone, 100_000).unwrap();
        assert_eq!(
            res.messages[0].msg,
            BankMsg::Send {
                to_address: "creator".to_string(),
                amount: coins(100_000, "uatom")
            }
            .into()
        );
    }
}
