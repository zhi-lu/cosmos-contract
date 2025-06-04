mod blackjack;
mod coin;
mod msg;
mod slot;
mod state;
mod utils;

use crate::blackjack::BlackjackAction;
use crate::coin::CoinSide;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::slot::Symbol;
use crate::state::{
    BlackjackState, BlackjackStateResponse, LockedAmountResponse, State, BLACKJACK_STATE, STATE,
};
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
        ExecuteMsg::PlayWar {} => play_war(deps, env, info),
        ExecuteMsg::PlaySlot {} => play_slot(deps, env, info),
        ExecuteMsg::GuessNumber { guess } => play_guess_number(deps, env, info, guess),
        ExecuteMsg::PlayBlackjack { action } => match action {
            BlackjackAction::Start => play_blackjack_start(deps, env, info),
            BlackjackAction::Hit => play_blackjack_hit(deps, env, info),
            BlackjackAction::Stand => play_blackjack_stand(deps, env, info),
        },
        ExecuteMsg::PlayCoinFlip { choice } => play_coin_flip(deps, env, info, choice),
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
        QueryMsg::GetBlackjackState { address } => {
            let addr = deps.api.addr_validate(&address);
            let state = BLACKJACK_STATE.load(deps.storage, &addr.unwrap())?;
            let mut hide_dealer_cards = state.dealer_cards;
            if !state.finished {
                hide_dealer_cards[0] = 0;
            }
            let resp = BlackjackStateResponse {
                user_cards: state.user_cards,
                dealer_cards: hide_dealer_cards,
                bet: state.bet,
                finished: state.finished,
            };
            to_json_binary(&resp)
        }
    }
}

/// 比大小游戏
///
/// 用户和合约进行比大小游戏, 用户生成的数字大于合约生成的数字，则用户获胜，获得下注金额 ×2 的奖励.
fn play_war(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
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
    let mut result = "lost";
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
        result = "win"
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
        result = "tie"
    }

    // 更新锁仓金额
    // 用户输：无需操作（资金留在合约）
    STATE.save(deps.storage, &state)?;

    Ok(response
        .add_attribute("action", "play_war")
        .add_attribute("user_rand", user_rand.to_string())
        .add_attribute("contract_rand", contract_rand.to_string())
        .add_attribute("result", result))
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

/// 猜数字游戏(范围 1 ～ 10)
///
/// 合约生成一个随机数，如果用户猜对，获得奖励。(完全猜中 x10、相邻 x1）
fn play_guess_number(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_guess: u8,
) -> StdResult<Response> {
    if user_guess < 1 || user_guess > 10 {
        return Err(StdError::generic_err("Guess must be between 1 and 10"));
    }

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

    // 锁仓金额
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += sent_amount;

    // 生成合约伪随机数 1~10
    let rand = utils::generate_random_number(&info, &env, b"guess") % 10 + 1;

    let mut payout = 0;
    let mut result = "lost";

    if user_guess as u32 == rand {
        payout = sent_amount * 10;
        result = "exact";
    } else if (user_guess as i32 - rand as i32).abs() == 1 {
        payout = sent_amount * 1;
        result = "adjacent";
    }

    let mut response = Response::new();

    if payout > 0 {
        response = response.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uatom".to_string(),
                amount: Uint128::from(payout),
            }],
        });

        state.locked_amount = state.locked_amount.saturating_sub(payout);
    }

    STATE.save(deps.storage, &state)?;

    Ok(response
        .add_attribute("action", "play_guess_number")
        .add_attribute("user_guess", user_guess.to_string())
        .add_attribute("correct_number", rand.to_string())
        .add_attribute("result", result))
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

/// 21 点游戏启动
///
/// 启动 21 点游戏, 用户下注金额必须介于 100,000 和 10,000,000 uatom 之间。
fn play_blackjack_start(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    // 验证下注金额
    let bet = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    if bet < 100_000 || bet > 10_000_000 {
        return Err(StdError::generic_err(
            "Bet must be between 100,000 and 10,000,000 uatom",
        ));
    }

    let bet_amount = Uint128::from(bet);

    // 生成 4 张初始牌: 2 张牌是用户的、2 张牌是庄家的
    let user_card1 = utils::generate_random_number(&info, &env, b"user1") % 10 + 1;
    let user_card2 = utils::generate_random_number(&info, &env, b"user2") % 10 + 1;
    let dealer_card1 = utils::generate_random_number(&info, &env, b"dealer1") % 10 + 1;
    let dealer_card2 = utils::generate_random_number(&info, &env, b"dealer2") % 10 + 1;

    // 保存游戏状态
    let state = BlackjackState {
        user_cards: vec![user_card1, user_card2],
        dealer_cards: vec![dealer_card1, dealer_card2],
        bet: bet_amount,
        finished: false,
    };

    BLACKJACK_STATE.save(deps.storage, &info.sender, &state)?;

    // 更新合约全局锁仓
    let mut global_state = STATE.load(deps.storage)?;
    global_state.locked_amount += bet;
    STATE.save(deps.storage, &global_state)?;

    // 构造返回
    Ok(Response::new()
        .add_attribute("action", "play_blackjack_start")
        .add_attribute("user_card1", user_card1.to_string())
        .add_attribute("user_card2", user_card2.to_string())
        .add_attribute("dealer_card1", "hide") // 庄家的起手牌进行 hide
        .add_attribute("dealer_card2", dealer_card2.to_string()))
}

/// 21 点玩家要牌
///
/// 当用户不超过 21 点数时, 用户可以继续要牌, 否则用户无法再要牌.
fn play_blackjack_hit(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    // 加载当前用户游戏状态
    let mut state: BlackjackState = BLACKJACK_STATE.load(deps.storage, &info.sender)?;

    if state.finished {
        return Err(StdError::generic_err("Game already finished"));
    }

    // 检查用户当前点数是否已经达到或超过 21，防止继续要牌
    let current_total: u32 = utils::calculate_blackjack_total(&state.user_cards);
    if current_total >= 21 {
        return Err(StdError::generic_err(
            "You cannot hit after reaching 21 points",
        ));
    }

    // 发一张新牌
    let new_card = utils::generate_random_number(&info, &env, b"hit_card") % 10 + 1;
    state.user_cards.push(new_card);

    // 更新状态
    BLACKJACK_STATE.save(deps.storage, &info.sender, &state)?;

    // 返回结果
    Ok(Response::new()
        .add_attribute("action", "blackjack_hit")
        .add_attribute("new_card", new_card.to_string())
        .add_attribute(
            "current_total",
            utils::calculate_blackjack_total(&state.user_cards).to_string(),
        ))
}

/// 21 点玩家停牌
///
/// 当玩家开始停牌时, 庄家会根据实际情况进行要牌.
fn play_blackjack_stand(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let mut state = BLACKJACK_STATE.load(deps.storage, &info.sender)?;
    if state.finished {
        return Err(StdError::generic_err("Game already finished"));
    }

    let user_total: u32 = utils::calculate_blackjack_total(&state.user_cards);
    let mut dealer_total: u32 = utils::calculate_blackjack_total(&state.dealer_cards);

    // 如果玩家爆牌, 直接结束游戏.
    if user_total > 21 {
        state.finished = true;
        BLACKJACK_STATE.save(deps.storage, &info.sender, &state)?;
        return Ok(Response::new()
            .add_attribute("action", "blackjack_stand")
            .add_attribute("result", "player_busted")
            .add_attribute("user_total", user_total.to_string())
            .add_attribute("dealer_total", dealer_total.to_string()));
    }

    // 庄家的补充牌逻辑: 小于 17 点或者小于用户的牌必须需要牌.
    while dealer_total < 17 || dealer_total < user_total {
        let new_card = utils::generate_random_number(&info, &env, b"dealer_hit") % 10 + 1;
        state.dealer_cards.push(new_card);
        dealer_total = utils::calculate_blackjack_total(&state.dealer_cards);
    }

    // 判断胜负
    let result: &str;
    let payout: Uint128;

    if dealer_total > 21 {
        // 玩家赢了
        result = "player_win";
        payout = state.bet * Uint128::new(2);
    } else if user_total < dealer_total {
        // 玩家输了
        result = "dealer_win";
        payout = Uint128::zero();
    } else {
        // 平局，退还本金
        result = "draw";
        payout = state.bet;
    }

    // 更新 locked_amount 锁仓状态
    let mut global_state = STATE.load(deps.storage)?;
    global_state.locked_amount = global_state.locked_amount.saturating_sub(payout.u128());
    STATE.save(deps.storage, &global_state)?;

    // 结束游戏.
    state.finished = true;
    BLACKJACK_STATE.save(deps.storage, &info.sender, &state)?;

    // 创建响应对象
    let mut response = Response::new();

    // 如果是平局或者玩家赢了, 发送支付金额给玩家
    if payout != Uint128::zero() {
        response = response.add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: "uatom".to_string(),
                amount: payout,
            }],
        });
    }

    // 返回结果
    Ok(response
        .add_attribute("action", "blackjack_stand")
        .add_attribute("result", result)
        .add_attribute("user_total", user_total.to_string())
        .add_attribute("dealer_total", dealer_total.to_string())
        .add_attribute("user_cards", format!("{:?}", state.user_cards))
        .add_attribute("dealer_cards", format!("{:?}", state.dealer_cards))
        .add_attribute("payout", payout.to_string()))
}

/// 玩硬币翻牌
///
/// 用户猜硬币的结果，如果猜对了，则获得 bet * 2 的金额，否则损失 bet 的金额。
fn play_coin_flip(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    choice: CoinSide,
) -> StdResult<Response> {
    // 验证下注金额
    let bet = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    if bet < 100_000 || bet > 10_000_000 {
        return Err(StdError::generic_err(
            "Bet must be between 100,000 and 10,000,000 uatom",
        ));
    }

    // 修改锁仓 lock_amount
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += bet;
    STATE.save(deps.storage, &state)?;

    // 抛硬币：0 -> Heads, 1 -> Tails
    let rand = utils::generate_random_number(&info, &env, b"coin_flip") % 2;
    let result = if rand == 0 {
        CoinSide::Heads
    } else {
        CoinSide::Tails
    };

    let mut response = Response::new()
        .add_attribute("action", "coin_flip")
        .add_attribute("player_choice", format!("{:?}", choice))
        .add_attribute("actual_result", format!("{:?}", result));

    if choice == result {
        // 赢了，奖励翻倍
        let payout = Uint128::from(bet) * Uint128::new(2);
        state.locked_amount = state.locked_amount.saturating_sub(payout.u128());
        STATE.save(deps.storage, &state)?;

        response = response
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: payout,
                }],
            })
            .add_attribute("result", "win")
            .add_attribute("payout", payout.to_string());
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{attr, coins, from_json};

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
    pub fn test_play_war() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        let res = play_war(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(100_000, "uatom")),
        )
        .unwrap();

        let attrs = res.attributes;
        let result = attrs
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        if "win" == result.value {
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: "user".to_string(),
                    amount: coins(200_000, "uatom")
                }
                .into()
            );
        } else if "tie" == result.value {
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

        // 检查返回的事件属性
        let attrs = res.attributes;

        // 如果用户赢了或平局，应该包含 payout_multiplier，否则包含 result = lost
        let payout_attr = attrs.iter().find(|a| a.key == "payout_multiplier");
        let result_attr = attrs.iter().find(|a| a.key == "result");

        // 确保包含下注金额
        let bet_attr = attrs
            .iter()
            .find(|a| a.key == "bet_amount")
            .expect("bet_amount missing");
        assert_eq!(bet_attr.value, "100000");

        // 至少应该有一个
        assert!(
            payout_attr.is_some() || result_attr.is_some(),
            "expected either payout_multiplier or result"
        );
    }

    #[test]
    pub fn test_play_guess_number() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = play_guess_number(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(100_000, "uatom")),
            9,
        )
        .unwrap();

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        let user_guess = res
            .attributes
            .iter()
            .find(|a| a.key == "user_guess")
            .expect("user_guess missing");

        let contract_guess = res
            .attributes
            .iter()
            .find(|a| a.key == "correct_number")
            .expect("contract_guess missing");

        // 如果用户猜的数字和 contract 生成的随机数字相同，则用户获取 10 倍奖励.
        if "exact" == result.value {
            assert_eq!(user_guess.value, contract_guess.value);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: "user".to_string(),
                    amount: coins(100_000 * 10, "uatom")
                }
                .into()
            )
        } else if "adjacent" == result.value {
            // 如果用户猜的数字和 contract 生成的随机数字相邻, 则用户获取 1 倍奖励
            let user_guess_num = user_guess.value.parse::<i32>().unwrap();
            let contract_guess_num = contract_guess.value.parse::<i32>().unwrap();
            assert_eq!((user_guess_num - contract_guess_num).abs(), 1);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: "user".to_string(),
                    amount: coins(100_000 * 1, "uatom")
                }
                .into()
            )
        } else {
            // 用户猜的数字与合约的数字既不相同也不相邻则不获取任何奖励.
            assert_eq!(res.messages.len(), 0);
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

    #[test]
    fn test_blackjack_full_flow() {
        let mut deps = mock_dependencies();

        // 初始化合约
        let instantiate_msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, instantiate_msg).unwrap();

        let player = "player1";

        // ----------------------------
        // Step 1: Start 游戏
        // ----------------------------
        let start_msg = ExecuteMsg::PlayBlackjack {
            action: BlackjackAction::Start,
        };
        let info = mock_info(player, &coins(500_000, "uatom")); // 有效下注金额
        let res = execute(deps.as_mut(), mock_env(), info.clone(), start_msg).unwrap();
        assert_eq!(res.attributes[0], attr("action", "play_blackjack_start"));

        let mut query_msg = QueryMsg::GetBlackjackState {
            address: player.to_string(),
        };

        let mut bin = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let mut resp: BlackjackStateResponse = from_json(&bin).unwrap();

        let user_total_by_resp: u32 = utils::calculate_blackjack_total(&resp.user_cards);
        let user_card1 = &res
            .attributes
            .iter()
            .find(|a| a.key == "user_card1")
            .unwrap()
            .value;
        let user_card2 = &res
            .attributes
            .iter()
            .find(|a| a.key == "user_card2")
            .unwrap()
            .value;
        let user_total_by_attr: u32 = utils::calculate_blackjack_total(&*vec![
            user_card1.parse::<u32>().unwrap(),
            user_card2.parse::<u32>().unwrap(),
        ]);
        assert_eq!(user_total_by_resp, user_total_by_attr);

        // ----------------------------
        // Step 2: 当用户的牌小于 17 点, Hit 要一张牌
        // ----------------------------

        if user_total_by_attr < 17 {
            let hit_msg = ExecuteMsg::PlayBlackjack {
                action: BlackjackAction::Hit,
            };

            let res = execute(deps.as_mut(), mock_env(), info.clone(), hit_msg).unwrap();

            assert_eq!(res.attributes[0], attr("action", "blackjack_hit"));
        }

        // ----------------------------
        // Step 3: Stand 停牌
        // ----------------------------
        let stand_msg = ExecuteMsg::PlayBlackjack {
            action: BlackjackAction::Stand,
        };
        let res = execute(deps.as_mut(), mock_env(), info.clone(), stand_msg).unwrap();
        assert_eq!(res.attributes[0], attr("action", "blackjack_stand"));

        query_msg = QueryMsg::GetBlackjackState {
            address: player.to_string(),
        };
        bin = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        resp = from_json(&bin).unwrap();

        assert!(resp.finished);
        assert!(!resp.user_cards.is_empty());
        assert!(!resp.dealer_cards.is_empty());
    }

    #[test]
    fn test_blackjack_query_state() {
        let mut deps = mock_dependencies();

        // 初始化合约
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();

        // 模拟用户参与 Blackjack 游戏
        let user = "player1";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let start_msg = ExecuteMsg::PlayBlackjack {
            action: BlackjackAction::Start,
        };

        let _res = execute(deps.as_mut(), mock_env(), user_info.clone(), start_msg).unwrap();

        // 查询 Blackjack 状态
        let query_msg = QueryMsg::GetBlackjackState {
            address: user.to_string(),
        };

        let bin = query(deps.as_ref(), mock_env(), query_msg).unwrap();
        let resp: BlackjackStateResponse = from_json(&bin).unwrap();

        // 验证结构内容
        assert_eq!(resp.user_cards.len(), 2);
        assert_eq!(resp.dealer_cards.len(), 2);
        assert_eq!(resp.dealer_cards[0], 0); // 未完成游戏，庄家第一张牌被隐藏
        assert_eq!(resp.bet, Uint128::new(1_000_000));
        assert_eq!(resp.finished, false);
    }

    #[test]
    fn test_play_coin_flip() {
        let mut deps = mock_dependencies();

        // 初始化合约
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();

        // 模拟用户参与 coin_flip 游戏
        let user = "player1";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let coin_flip = ExecuteMsg::PlayCoinFlip {
            choice: CoinSide::Heads,
        };

        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), coin_flip).unwrap();

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        if "win" == result.value {
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(2_000_000, "uatom")
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }
}
