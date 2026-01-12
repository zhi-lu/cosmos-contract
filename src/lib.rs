mod baccarat;
mod blackjack;
mod coin;
mod dice;
mod msg;
mod roulette;
mod slot;
mod state;
mod utils;

use crate::baccarat::BaccaratBet;
use crate::blackjack::BlackjackAction;
use crate::coin::CoinSide;
use crate::dice::{DiceGameMode, DiceGuessSize};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::roulette::{Color, EvenOdd, HighLow, RouletteBetType, RouletteResult};
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
    // 初始化需要的最少锁仓金额
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
    // 检查合约是否至少有 100,000,000 uatom 锁仓
    let state = STATE.load(deps.storage)?;
    if state.locked_amount < 100_000_000 {
        return Err(StdError::generic_err(
            "Contract must have at least 100,000,000 uatom locked",
        ));
    }
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
        ExecuteMsg::PlayDice { mode } => match mode {
            DiceGameMode::GuessSize { guess_big } => {
                play_dice_guess_size(deps, env, info, guess_big)
            }
            DiceGameMode::ExactNumber { guess_number } => {
                play_dice_exact_number(deps, env, info, guess_number)
            }
            DiceGameMode::RangeBet { start, end } => {
                play_dice_range_bet(deps, env, info, start, end)
            }
        },
        ExecuteMsg::PlayBaccarat { bet_choice } => play_baccarat(deps, env, info, bet_choice),
        ExecuteMsg::PlayRoulette { bet_type } => play_roulette(deps, env, info, bet_type),
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
/// 用户和合约进行比大小游戏, 用户生成的数字大于合约生成的数字,则用户获胜,获得下注金额 ×2 的奖励.
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
        // 用户赢: 发送奖励（下注金额 ×2）
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
        // 平局: 退还下注
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
    // 用户输: 无需操作（资金留在合约）
    STATE.save(deps.storage, &state)?;

    Ok(response
        .add_attribute("action", "play_war")
        .add_attribute("user_rand", user_rand.to_string())
        .add_attribute("contract_rand", contract_rand.to_string())
        .add_attribute("result", result))
}

/// 老虎机游戏
///
/// slot 的游戏, 如果用户中了 3 个相同符号,则用户中了该符号的全部奖励, 如果中了 2 个相同符号,则用户中了该符号的 1/2 的奖励, 否则啥奖励都没有
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
        // 三个相同: 获取全额奖励
        payout_multiplier = symbol1.payout_multiplier();
    } else if symbol1 == symbol2 || symbol1 == symbol3 || symbol2 == symbol3 {
        // 任意两个相同: 获取半额奖励
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
/// 合约生成一个随机数,如果用户猜对,获得奖励。(完全猜中 x10、相邻 x1）
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

    // 检查用户当前点数是否已经达到或超过 21,防止继续要牌
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
        // 平局,退还本金
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
/// 用户猜硬币的结果,如果猜对了,则获得 bet * 2 的金额,否则损失 bet 的金额。
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

    // 抛硬币: 0 -> Heads, 1 -> Tails
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
        // 赢了,奖励翻倍
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

/// 玩骰子猜大小
///
/// 用户猜中大小的概率为 1/2, 用户猜中获得 bet * 2 的金额, 否则损失 bet 的金额.
fn play_dice_guess_size(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    guess_big: DiceGuessSize,
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

    // 抛骰子: [1,3] 为小, [4,6] 为大
    let rand_number = utils::generate_random_number(&info, &env, b"dice_guess_size") % 6 + 1;
    let result = if rand_number <= 3 {
        DiceGuessSize::Small
    } else {
        DiceGuessSize::Big
    };

    let mut response = Response::new()
        .add_attribute("action", "dice_guess_size")
        .add_attribute("player_guess", format!("{:?}", guess_big))
        .add_attribute("actual_result", format!("{:?}", result));

    if guess_big == result {
        // 赢了发送奖励
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
            .add_attribute("result", "win");
    } else {
        response = response.add_attribute("result", "lose");
    }
    Ok(response)
}

/// 玩骰子猜点数
///
/// 用户猜中数字的概率为 1/6, 猜中数字的奖励为 bet * 6 的金额, 否则损失 bet 的金额.
fn play_dice_exact_number(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    number: u8,
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

    if number < 1 || number > 6 {
        return Err(StdError::generic_err("Number must be between 1 and 6"));
    }

    // 修改锁仓 lock_amount
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += bet;
    STATE.save(deps.storage, &state)?;

    // 抛骰子
    let rand_number = utils::generate_random_number(&info, &env, b"dice_exact_number") % 6 + 1;

    let mut response = Response::new()
        .add_attribute("action", "play_dice_exact_number")
        .add_attribute("player_guess", number.to_string())
        .add_attribute("actual_result", rand_number.to_string());

    // 如果猜对,玩家则获得 bet * 6 的金额,否则损失 bet 的金额。
    if number as u32 == rand_number {
        let payout = Uint128::from(bet) * Uint128::new(6);
        state.locked_amount = state.locked_amount.saturating_sub(payout.u128());
        STATE.save(deps.storage, &state)?;

        // 赢了发送奖励
        response = response
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: payout,
                }],
            })
            .add_attribute("payout", payout.to_string())
            .add_attribute("result", "win");
    } else {
        response = response.add_attribute("result", "lose");
    }
    Ok(response)
}

/// 玩骰子猜范围
///
/// 用户在指定范围内猜骰子点, 猜中范围的概率为 1 / (6 / ( end - start + 1 )), 猜中范围的奖励为 bet * times 的金额, 否则损失 bet 的金额.
fn play_dice_range_bet(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    start: u8,
    end: u8,
) -> StdResult<Response> {
    if start > end || start < 1 || start > 6 || end < 1 || end > 6 {
        return Err(StdError::generic_err("Invalid range"));
    }

    let width = end - start + 1;
    let times = match width {
        2 => 3,
        3 => 2,
        _ => return Err(StdError::generic_err("Range width must be 2 or 3")),
    };

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

    let rand_number = utils::generate_random_number(&info, &env, b"dice_range_bet") % 6 + 1;

    if rand_number >= start as u32 && rand_number <= end as u32 {
        let payout = Uint128::from(bet) * Uint128::new(times);
        state.locked_amount = state.locked_amount.saturating_sub(payout.u128());
        STATE.save(deps.storage, &state)?;

        return Ok(Response::new()
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: payout,
                }],
            })
            .add_attribute("result", "win")
            .add_attribute("payout", payout.to_string())
            .add_attribute("actual_result", rand_number.to_string())
            .add_attribute("player_start", start.to_string())
            .add_attribute("player_end", end.to_string()));
    }
    Ok(Response::new()
        .add_attribute("result", "lose")
        .add_attribute("actual_result", rand_number.to_string())
        .add_attribute("player_start", start.to_string())
        .add_attribute("player_end", end.to_string()))
}

/// 百家乐游戏
///
/// 玩家可以在庄家、闲家或平局中选择下注
fn play_baccarat(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bet_choice: BaccaratBet,
) -> StdResult<Response> {
    // 检查下注金额
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

    // 更新合约锁仓金额
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += bet;
    STATE.save(deps.storage, &state)?;

    // 发牌 - 百家乐规则：每人先发两张牌
    let mut player_cards = vec![
        (utils::generate_random_number(&info, &env, b"player1") % 10) as u8,
        (utils::generate_random_number(&info, &env, b"player2") % 10) as u8,
    ];

    let mut banker_cards = vec![
        (utils::generate_random_number(&info, &env, b"banker1") % 10) as u8,
        (utils::generate_random_number(&info, &env, b"banker2") % 10) as u8,
    ];

    // 计算点数（百家乐中只有个位数有效）
    let mut player_total = (player_cards[0] + player_cards[1]) % 10;
    let mut banker_total = (banker_cards[0] + banker_cards[1]) % 10;

    // 根据规则决定是否补牌
    let player_third_card = if player_total <= 5 {
        let third_card = (utils::generate_random_number(&info, &env, b"player3") % 10) as u8;
        player_cards.push(third_card);
        player_total = (player_total + third_card) % 10;
        Some(third_card)
    } else {
        None
    };

    // 庄家是否补牌取决于闲家是否补牌以及当前点数
    if banker_total <= 5 {
        let should_draw = match player_third_card {
            Some(third_card) => {
                // 根据百家乐规则确定庄家是否补牌
                match banker_total {
                    0..=2 => true,                                    // 庄家0-2点必补牌
                    3 => third_card != 8, // 庄家3点，闲家第三张为8时不补牌
                    4 => matches!(third_card, 2 | 3 | 4 | 5 | 6 | 7), // 庄家4点规则
                    5 => matches!(third_card, 4 | 5 | 6 | 7), // 庄家5点规则
                    6 => matches!(third_card, 6 | 7), // 庄家6点规则
                    _ => false,           // 庄家7点以上不补牌
                }
            }
            None => banker_total <= 5, // 如果闲家没补牌，庄家按基本规则补牌
        };

        if should_draw {
            let third_card = (utils::generate_random_number(&info, &env, b"banker3") % 10) as u8;
            banker_cards.push(third_card);
            banker_total = (banker_total + third_card) % 10;
        }
    }

    // 确定赢家
    let winner = if player_total > banker_total {
        BaccaratBet::Player
    } else if banker_total > player_total {
        BaccaratBet::Banker
    } else {
        BaccaratBet::Tie
    };

    // 计算赔付
    let (payout_multiplier, commission) = match winner {
        BaccaratBet::Player => (2, 0), // 1:1 赔付，无佣金
        BaccaratBet::Banker => (2, 5), // 1:1 赔付，但收取5%佣金
        BaccaratBet::Tie => (9, 0),    // 8:1 赔付 (我们设置为9倍因为包含本金)
    };

    let mut response = Response::new()
        .add_attribute("action", "play_baccarat")
        .add_attribute("player_cards", format!("{:?}", player_cards))
        .add_attribute("banker_cards", format!("{:?}", banker_cards))
        .add_attribute("player_total", player_total.to_string())
        .add_attribute("banker_total", banker_total.to_string())
        .add_attribute("player_bet", format!("{:?}", bet_choice))
        .add_attribute("winner", format!("{:?}", winner));

    // 如果玩家猜中了结果
    if winner == bet_choice {
        let mut winnings = bet * (payout_multiplier as u128 - 1); // 奖金不包括本金

        // 如果投注庄家且获胜，扣除佣金
        if winner == BaccaratBet::Banker {
            let commission_amount = winnings * commission / 100;
            winnings = winnings.saturating_sub(commission_amount);
        }

        // 特殊处理平局情况
        if winner == BaccaratBet::Tie {
            winnings = bet * 8; // 8倍奖金
        }

        let payout_amount = bet + winnings; // 本金+奖金
        let payout = Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(payout_amount),
        };

        response = response
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![payout],
            })
            .add_attribute("result", "win")
            .add_attribute("winnings", winnings.to_string())
            .add_attribute("payout", payout_amount.to_string());

        state.locked_amount = state.locked_amount.saturating_sub(payout_amount);
        STATE.save(deps.storage, &state)?;
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    Ok(response)
}

/// 轮盘游戏
///
/// 轮盘包含数字 0-36，其中：
/// - 0 用于显示颜色但不参与颜色押注
/// - 仅保留四种玩法：单个数字、颜色、奇偶、大小
fn play_roulette(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bet_type: RouletteBetType,
) -> StdResult<Response> {
    // 检查下注金额
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

    // 更新合约锁仓金额
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += bet;
    STATE.save(deps.storage, &state)?;

    // 生成 0-36 的随机数作为轮盘结果
    let winning_number = (utils::generate_random_number(&info, &env, b"roulette") % 37) as u8;

    // 根据轮盘规则确定颜色
    let winning_color = get_roulette_color(winning_number);

    // 判断是否为偶数
    let is_even = winning_number != 0 && winning_number % 2 == 0;

    // 判断大小（0 不属于任何一类）
    let is_low = if winning_number == 0 {
        None
    } else if winning_number <= 18 {
        Some(true) // Low (1-18)
    } else {
        Some(false) // High (19-36)
    };

    // 创建结果对象
    let result = RouletteResult {
        winning_number,
        winning_color: winning_color.clone(),
        is_even,
        is_low,
    };

    // 计算赔付
    let (won, payout_multiplier) = calculate_roulette_payout(&bet_type, &result);

    let mut response = Response::new()
        .add_attribute("action", "play_roulette")
        .add_attribute("winning_number", winning_number.to_string())
        .add_attribute("winning_color", format!("{:?}", winning_color))
        .add_attribute("is_even", is_even.to_string())
        .add_attribute("bet_type", format!("{:?}", bet_type));

    if won {
        let winnings = bet * payout_multiplier;
        let payout = Coin {
            denom: "uatom".to_string(),
            amount: Uint128::from(winnings),
        };

        response = response
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![payout],
            })
            .add_attribute("result", "win")
            .add_attribute("winnings", winnings.to_string());

        state.locked_amount = state.locked_amount.saturating_sub(winnings);
        STATE.save(deps.storage, &state)?;
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    Ok(response)
}

/// 获取轮盘数字的颜色
fn get_roulette_color(number: u8) -> Color {
    if number == 0 {
        // 0 仅用于显示为绿色/特殊色，颜色押注不生效
        Color::Black // 若 Color 枚举无绿色，保持显示但不允许颜色押注中奖
    } else {
        // 轮盘红色数字: 1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36
        match number {
            1 | 3 | 5 | 7 | 9 | 12 | 14 | 16 | 18 | 19 | 21 | 23 | 25 | 27 | 30 | 32 | 34 | 36 => {
                Color::Red
            }
            _ => Color::Black, // 其余为黑色
        }
    }
}

/// 计算轮盘游戏的赔付
fn calculate_roulette_payout(bet_type: &RouletteBetType, result: &RouletteResult) -> (bool, u128) {
    match bet_type {
        // 单个数字下注，赔率 35:1
        RouletteBetType::SingleNumber { number } => {
            if *number == result.winning_number {
                (true, 36) // 包含本金的总倍数
            } else {
                (false, 0)
            }
        }
        // 颜色下注，赔率 1:1；开出 0 时颜色押注必输
        RouletteBetType::Color { color } => {
            if result.winning_number != 0 && *color == result.winning_color {
                (true, 2)
            } else {
                (false, 0)
            }
        }
        // 奇偶下注，赔率 1:1
        RouletteBetType::EvenOdd { bet } => {
            // 0 既不是奇数也不是偶数，投注奇偶都不会中奖
            if result.winning_number == 0 {
                (false, 0)
            } else {
                match bet {
                    EvenOdd::Even => (result.is_even, 2), // 包含本金的总倍数
                    EvenOdd::Odd => (!result.is_even, 2), // 包含本金的总倍数
                }
            }
        }

        // 大小下注（1-18 / 19-36），赔率 1:1
        RouletteBetType::HighLow { bet } => {
            if let Some(is_low) = result.is_low {
                match bet {
                    HighLow::Low => (is_low, 2),   // 包含本金的总倍数
                    HighLow::High => (!is_low, 2), // 包含本金的总倍数
                }
            } else {
                // 如果是0，大小投注都不中奖
                (false, 0)
            }
        }
    }
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

        // 如果用户赢了或平局,应该包含 payout_multiplier,否则包含 result = lost
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

        // 如果用户猜的数字和 contract 生成的随机数字相同,则用户获取 10 倍奖励.
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
        assert_eq!(resp.dealer_cards[0], 0); // 未完成游戏,庄家第一张牌被隐藏
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

    #[test]
    fn test_play_dice_guess_size() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();
        let user = "player1";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));
        let dice_guess = ExecuteMsg::PlayDice {
            mode: DiceGameMode::GuessSize {
                guess_big: DiceGuessSize::Small,
            },
        };

        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), dice_guess).unwrap();
        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        let player_guess = res
            .attributes
            .iter()
            .find(|a| a.key == "player_guess")
            .expect("player_guess missing");

        let actual_result = res
            .attributes
            .iter()
            .find(|a| a.key == "actual_result")
            .expect("actual_result missing");

        if result.value == "win" {
            assert_eq!(player_guess.value, actual_result.value);
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

    #[test]
    fn test_play_dice_guess_number() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();
        let user = "player1";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));
        let dice_guess = ExecuteMsg::PlayDice {
            mode: DiceGameMode::ExactNumber { guess_number: 6 },
        };

        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), dice_guess).unwrap();

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        let player_guess = res
            .attributes
            .iter()
            .find(|a| a.key == "player_guess")
            .expect("player_guess missing");

        let actual_result = res
            .attributes
            .iter()
            .find(|a| a.key == "actual_result")
            .expect("actual_result missing");

        if result.value == "win" {
            assert_eq!(player_guess.value, actual_result.value);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(6_000_000, "uatom")
                }
                .into()
            )
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_dice_range_bet() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(20_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();
        let user = "player1";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));
        let dice_guess = ExecuteMsg::PlayDice {
            mode: DiceGameMode::RangeBet { start: 1, end: 3 },
        };
        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), dice_guess).unwrap();
        print!("{:?}", res);

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        let actual_result = res
            .attributes
            .iter()
            .find(|a| a.key == "actual_result")
            .expect("actual_result missing");

        let player_start = res
            .attributes
            .iter()
            .find(|a| a.key == "player_start")
            .expect("player_start missing");

        let player_end = res
            .attributes
            .iter()
            .find(|a| a.key == "player_end")
            .expect("player_end missing");

        if result.value == "win" {
            let actual_result_int = actual_result.value.parse::<u32>().unwrap();
            assert_eq!(
                true,
                actual_result_int >= player_start.value.parse::<u32>().unwrap()
            );
            assert_eq!(
                true,
                actual_result_int <= player_end.value.parse::<u32>().unwrap()
            );
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(2_000_000, "uatom")
                }
                .into()
            )
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_baccarat() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();
        let user = "player1";

        // 测试玩家获胜情况
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));
        let baccarat_game = ExecuteMsg::PlayBaccarat {
            bet_choice: BaccaratBet::Player,
        };
        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), baccarat_game).unwrap();

        println!("{:?}", res);

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        let winner = res
            .attributes
            .iter()
            .find(|a| a.key == "winner")
            .expect("winner missing");

        if result.value == "win" {
            // 如果玩家获胜，检查是否正确赔付 (1:1 奖金 + 1:1 本金 = 2倍)
            if winner.value == "Player" {
                assert_eq!(
                    res.messages[0].msg,
                    BankMsg::Send {
                        to_address: user.to_string(),
                        amount: coins(2_000_000, "uatom")
                    }
                    .into()
                );
            } else if winner.value == "Banker" {
                // 庄家获胜但是用户押注错了
                assert_eq!(res.messages.len(), 0);
            } else {
                // 平局，赔率更高 (8:1 奖金 + 1:1 本金 = 9倍)
                assert_eq!(
                    res.messages[0].msg,
                    BankMsg::Send {
                        to_address: user.to_string(),
                        amount: coins(9_000_000, "uatom")
                    }
                    .into()
                );
            }
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_roulette() {
        let mut deps = mock_dependencies();
        let instantiate_msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, instantiate_msg).unwrap();
        let user = "player1";

        // 测试单数字投注
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));
        let roulette_game = ExecuteMsg::PlayRoulette {
            bet_type: RouletteBetType::SingleNumber { number: 17 },
        };
        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), roulette_game).unwrap();

        println!("{:?}", res);

        let result_single_number = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        if result_single_number.value == "win" {
            // 如果玩家获胜，检查是否正确赔付 (35:1 奖金 + 1:1 本金 = 36倍)
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(36_000_000, "uatom")
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }

        // 测试红色投注
        let user_info = mock_info(user, &coins(2_000_000, "uatom"));
        let roulette_game = ExecuteMsg::PlayRoulette {
            bet_type: RouletteBetType::Color { color: Color::Red },
        };
        let res = execute(deps.as_mut(), mock_env(), user_info.clone(), roulette_game).unwrap();

        println!("{:?}", res);

        let result_color = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        if result_color.value == "win" {
            // 如果玩家获胜，检查是否正确赔付 (1:1 奖金 + 1:1 本金 = 2倍)
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(4_000_000, "uatom")
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }
}
