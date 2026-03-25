mod baccarat;
mod blackjack;
mod bullfight;
mod coin;
mod dice;
mod keno;
mod msg;
mod omaha;
mod roulette;
mod sangong;
mod scratch;
mod sicbo;
mod slot;
mod state;
mod texas;
mod utils;

use crate::baccarat::BaccaratBet;
use crate::blackjack::BlackjackAction;
use crate::bullfight::{
    bull_hand_type_name, bull_payout_multiplier, evaluate_bull_hand, BullCard,
};
use crate::coin::CoinSide;
use crate::dice::{DiceGameMode, DiceGuessSize};
use crate::keno::{calculate_hits, keno_payout_multiplier, validate_picks};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::omaha::{
    best_omaha_hand_rank, hand_rank_name, Card, OmahaAction, OmahaState, OmahaStateResponse,
    OmahaStage,
};
use crate::roulette::{Color, EvenOdd, HighLow, RouletteBetType, RouletteResult};
use crate::sangong::{
    evaluate_sangong_hand, hand_type_name, payout_multiplier as sangong_payout_multiplier,
    SanGongCard, SanGongHandType,
};
use crate::scratch::{
    bet_range as scratch_bet_range, evaluate_scratch_card, ScratchCardType, ScratchSymbol,
};
use crate::sicbo::{calculate_sicbo_payout, validate_bet, SicBoBetType, SicBoResult};
use crate::slot::{evaluate_advanced, evaluate_basic, evaluate_mega, Symbol, SlotMode};
use crate::state::{
    BlackjackState, BlackjackStateResponse, LockedAmountResponse, State, BLACKJACK_STATE,
    OMAHA_STATE, STATE, TEXAS_STATE,
};
use crate::texas::{
    best_texas_hand_rank, TexasAction, TexasState, TexasStateResponse, TexasStage,
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
        ExecuteMsg::PlaySlot { mode } => play_slot(deps, env, info, mode),
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
        ExecuteMsg::PlayOmaha { action } => play_omaha(deps, env, info, action),
        ExecuteMsg::PlayTexas { action } => play_texas(deps, env, info, action),
        ExecuteMsg::PlaySanGong {} => play_sangong(deps, env, info),
        ExecuteMsg::PlaySicBo { bet_type } => play_sicbo(deps, env, info, bet_type),
        ExecuteMsg::PlayKeno { picks } => play_keno(deps, env, info, picks),
        ExecuteMsg::PlayScratchCard { card_type } => play_scratch_card(deps, env, info, card_type),
        ExecuteMsg::PlayBullFight {} => play_bullfight(deps, env, info),
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
        QueryMsg::GetOmahaState { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let state = OMAHA_STATE.load(deps.storage, &addr)?;
            let dealer_hand = if state.finished {
                state.dealer_hand.clone()
            } else {
                vec![] // 未结束时隐藏庄家手牌
            };
            let resp = OmahaStateResponse {
                player_hand: state.player_hand,
                dealer_hand,
                community_cards: state.community_cards,
                player_total_bet: state.player_total_bet,
                current_call_amount: state.current_call_amount,
                stage: state.stage,
                finished: state.finished,
            };
            to_json_binary(&resp)
        }
        QueryMsg::GetTexasState { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let state = TEXAS_STATE.load(deps.storage, &addr)?;
            let dealer_hand = if state.finished {
                state.dealer_hand.clone()
            } else {
                vec![] // 未结束时隐藏庄家手牌
            };
            let resp = TexasStateResponse {
                player_hand: state.player_hand,
                dealer_hand,
                community_cards: state.community_cards,
                player_total_bet: state.player_total_bet,
                current_call_amount: state.current_call_amount,
                stage: state.stage,
                finished: state.finished,
                all_in: state.all_in,
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
/// Slot 游戏
///
/// Basic 模式  (mode = basic)  : 3 轮 × 1 行
///   - 3 个相同符号（Wild 可替代）→ 全额倍率
///   - 2 个相同符号              → 半额倍率
///   - 全不同                    → 无奖励
///
/// Advanced 模式 (mode = advanced) : 5 轮 × 3 行，5 条赢线
///   - 每条赢线从左起连续 3/4/5 个相同（Wild 可替代）→ 倍率累加
///   - Scatter 出现 3/4/5+ 个 → 额外奖励倍率 5/15/50
///   - 总倍率 = 所有赢线倍率 + Scatter 奖励倍率
///
/// 下注范围：Basic 100,000 – 10,000,000 uatom
///          Advanced 200,000 – 10,000,000 uatom（5 线消耗更高）
fn play_slot(deps: DepsMut, env: Env, info: MessageInfo, mode: SlotMode) -> StdResult<Response> {
    // ── 验证投注金额 ──────────────────────────────
    let sent_amount = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    let (min_bet, max_bet) = match mode {
        SlotMode::Basic    => (100_000u128,  10_000_000u128),
        SlotMode::Advanced => (200_000u128,  10_000_000u128),
        SlotMode::Mega     => (500_000u128,  10_000_000u128),
    };

    if sent_amount < min_bet || sent_amount > max_bet {
        return Err(StdError::generic_err(format!(
            "Bet must be between {} and {} uatom for {:?} mode",
            min_bet, max_bet, mode
        )));
    }

    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += sent_amount;

    // ── 生成随机数并构建符号 ──────────────────────
    let mut response = Response::new()
        .add_attribute("mode", format!("{:?}", mode))
        .add_attribute("bet_amount", sent_amount.to_string());

    let payout_multiplier: u64;

    match mode {
        // ── Basic：3 轮 1 行 ─────────────────────
        SlotMode::Basic => {
            let r1 = utils::generate_random_number(&info, &env, b"slot_b1");
            let r2 = utils::generate_random_number(&info, &env, b"slot_b2");
            let r3 = utils::generate_random_number(&info, &env, b"slot_b3");

            let s1 = Symbol::from_u8(r1);
            let s2 = Symbol::from_u8(r2);
            let s3 = Symbol::from_u8(r3);

            response = response
                .add_attribute("reel1", format!("{:?}", s1))
                .add_attribute("reel2", format!("{:?}", s2))
                .add_attribute("reel3", format!("{:?}", s3));

            let result = evaluate_basic(&s1, &s2, &s3);
            payout_multiplier = result.multiplier;
            response = response.add_attribute("win_desc", result.description);
        }

        // ── Advanced：5 轮 3 行 5 赢线 ───────────
        SlotMode::Advanced => {
            // 生成 5 列 × 3 行 = 15 个随机数
            let salts: &[&[u8]; 15] = &[
                b"adv00", b"adv01", b"adv02",
                b"adv10", b"adv11", b"adv12",
                b"adv20", b"adv21", b"adv22",
                b"adv30", b"adv31", b"adv32",
                b"adv40", b"adv41", b"adv42",
            ];

            let mut rands = [0u32; 15];
            for (i, salt) in salts.iter().enumerate() {
                rands[i] = utils::generate_random_number(&info, &env, salt);
            }

            // 构建 grid[col][row]
            let grid: [[Symbol; 3]; 5] = [
                [Symbol::from_u8(rands[0]),  Symbol::from_u8(rands[1]),  Symbol::from_u8(rands[2])],
                [Symbol::from_u8(rands[3]),  Symbol::from_u8(rands[4]),  Symbol::from_u8(rands[5])],
                [Symbol::from_u8(rands[6]),  Symbol::from_u8(rands[7]),  Symbol::from_u8(rands[8])],
                [Symbol::from_u8(rands[9]),  Symbol::from_u8(rands[10]), Symbol::from_u8(rands[11])],
                [Symbol::from_u8(rands[12]), Symbol::from_u8(rands[13]), Symbol::from_u8(rands[14])],
            ];

            // 输出每列每行到 attributes
            for col in 0..5usize {
                for row in 0..3usize {
                    response = response.add_attribute(
                        format!("reel{}_{}", col + 1, row + 1),
                        format!("{:?}", grid[col][row]),
                    );
                }
            }

            let (total_mult, descriptions) = evaluate_advanced(&grid);
            payout_multiplier = total_mult;
            response = response.add_attribute("win_desc", descriptions.join("|"));
        }

        // ── Mega：6 轮 4 行 10 赢线 + 免费旋转 + Jackpot ──
        SlotMode::Mega => {
            // 生成 6 列 × 4 行 = 24 个随机数
            let salts: &[&[u8]; 24] = &[
                b"meg00", b"meg01", b"meg02", b"meg03",
                b"meg10", b"meg11", b"meg12", b"meg13",
                b"meg20", b"meg21", b"meg22", b"meg23",
                b"meg30", b"meg31", b"meg32", b"meg33",
                b"meg40", b"meg41", b"meg42", b"meg43",
                b"meg50", b"meg51", b"meg52",
                b"meg53",
            ];

            let mut rands = [0u32; 24];
            for (i, salt) in salts.iter().enumerate() {
                rands[i] = utils::generate_random_number(&info, &env, salt);
            }

            // 构建 grid[col][row]，6 列 × 4 行
            let grid: [[Symbol; 4]; 6] = [
                [Symbol::from_u8(rands[0]),  Symbol::from_u8(rands[1]),  Symbol::from_u8(rands[2]),  Symbol::from_u8(rands[3])],
                [Symbol::from_u8(rands[4]),  Symbol::from_u8(rands[5]),  Symbol::from_u8(rands[6]),  Symbol::from_u8(rands[7])],
                [Symbol::from_u8(rands[8]),  Symbol::from_u8(rands[9]),  Symbol::from_u8(rands[10]), Symbol::from_u8(rands[11])],
                [Symbol::from_u8(rands[12]), Symbol::from_u8(rands[13]), Symbol::from_u8(rands[14]), Symbol::from_u8(rands[15])],
                [Symbol::from_u8(rands[16]), Symbol::from_u8(rands[17]), Symbol::from_u8(rands[18]), Symbol::from_u8(rands[19])],
                [Symbol::from_u8(rands[20]), Symbol::from_u8(rands[21]), Symbol::from_u8(rands[22]), Symbol::from_u8(rands[23])],
            ];

            // 输出每列每行到 attributes
            for col in 0..6usize {
                for row in 0..4usize {
                    response = response.add_attribute(
                        format!("reel{}_{}", col + 1, row + 1),
                        format!("{:?}", grid[col][row]),
                    );
                }
            }

            let mega_result = evaluate_mega(&grid);
            payout_multiplier = mega_result.total_multiplier;
            let mut desc_parts = mega_result.descriptions;
            if mega_result.free_spin_triggered {
                desc_parts.push(format!("free_spin:triggered(x{})", mega_result.free_spin_multiplier));
            }
            if mega_result.jackpot {
                desc_parts.push("jackpot:true".to_string());
            }
            response = response
                .add_attribute("win_desc", desc_parts.join("|"))
                .add_attribute("free_spin", mega_result.free_spin_triggered.to_string())
                .add_attribute("jackpot", mega_result.jackpot.to_string());
        }
    }

    // ── 结算 ──────────────────────────────────────
    if payout_multiplier > 0 {
        let payout_amount = sent_amount * payout_multiplier as u128;

        // 防止合约余额不足时超额赔付
        if payout_amount > state.locked_amount {
            return Err(StdError::generic_err("Contract has insufficient funds for payout"));
        }

        state.locked_amount -= payout_amount;
        response = response
            .add_attribute("result", "win")
            .add_attribute("payout_multiplier", payout_multiplier.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout_amount),
                }],
            });
    } else {
        response = response.add_attribute("result", "lost");
    }

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

// ──────────────────────────────────────────────────────────────────────────────
// 奥马哈扑克（Omaha Hold'em）游戏
//
// 游戏流程：
//   1. Start        → 玩家下注底注，发 4 张手牌（庄家也发 4 张），进入 PreFlop
//   2. Raise/Call   → 在 PreFlop / Flop / Turn 阶段追加/跟注
//   3. Showdown     → 任意阶段直接摊牌结算
//   4. Fold         → 放弃本局，损失已押注金额
//
// 公共牌揭示节奏：
//   PreFlop  → Flop（3 张）→ Turn（+1 张）→ River（+1 张）→ Showdown
//
// 奥马哈规则：必须用恰好 2 张手牌 + 3 张公共牌组成最佳 5 张
// 加注规则：
//   - 加注 (Raise): 附带 funds 金额作为追加注额，当前跟注额 += raise 金额
//   - 跟注 (Call): 附带 funds 补齐差额（不少于 current_call_amount - player_total_bet）
//   - Showdown/Fold 时无需再附带 funds
// ──────────────────────────────────────────────────────────────────────────────
fn play_omaha(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: OmahaAction,
) -> StdResult<Response> {
    match action {
        // ── 开始游戏 ──────────────────────────────────────────────────
        OmahaAction::Start => {
            // 检查是否已有进行中游戏
            if let Ok(existing) = OMAHA_STATE.load(deps.storage, &info.sender) {
                if !existing.finished {
                    return Err(StdError::generic_err(
                        "You already have an active Omaha game. Fold or Showdown first.",
                    ));
                }
            }

            let bet = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if bet < 100_000 || bet > 5_000_000 {
                return Err(StdError::generic_err(
                    "Initial bet must be between 100,000 and 5,000,000 uatom",
                ));
            }

            // 生成洗牌后的牌组（0..=51 随机排列）
            let deck = shuffle_deck(&info, &env);

            // 发牌：玩家 4 张 (pos 0-3)，庄家 4 张 (pos 4-7)
            // 公共牌 5 张 (pos 8-12)，先全部生成但按阶段揭示
            let player_hand: Vec<Card> = (0..4).map(|i| Card::from_id(deck[i])).collect();
            let dealer_hand: Vec<Card> = (4..8).map(|i| Card::from_id(deck[i])).collect();
            // 公共牌全部预生成在 deck[8..13]，按阶段通过 advance_stage 揭示
            let state = OmahaState {
                player_hand: player_hand.clone(),
                dealer_hand,
                community_cards: vec![],
                player_total_bet: Uint128::from(bet),
                current_call_amount: Uint128::from(bet),
                stage: OmahaStage::PreFlop,
                finished: false,
                deck: deck.clone(),
                deck_pos: 13, // 前 13 张已用
            };

            OMAHA_STATE.save(deps.storage, &info.sender, &state)?;

            // 同时存储全部公共牌到 deck 的位置已固定，community 按阶段从 deck 取
            // 更新合约锁仓
            let mut global_state = STATE.load(deps.storage)?;
            global_state.locked_amount += bet;
            STATE.save(deps.storage, &global_state)?;

            Ok(Response::new()
                .add_attribute("action", "omaha_start")
                .add_attribute("stage", "PreFlop")
                .add_attribute("player_hand", format_cards(&player_hand))
                .add_attribute("community_cards", "[]")
                .add_attribute("initial_bet", bet.to_string()))
        }

        // ── 加注 ──────────────────────────────────────────────────────
        OmahaAction::Raise { amount } => {
            let mut state = OMAHA_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if matches!(state.stage, OmahaStage::Showdown) {
                return Err(StdError::generic_err("Game is at Showdown, cannot raise"));
            }

            // 检查附带的 funds
            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if sent < amount || amount == 0 {
                return Err(StdError::generic_err(
                    "Must attach exactly the raise amount in uatom funds",
                ));
            }

            if amount < 50_000 {
                return Err(StdError::generic_err("Minimum raise is 50,000 uatom"));
            }

            // 推进阶段并揭示公共牌
            let (new_stage, community) = advance_stage(&state);

            state.player_total_bet += Uint128::from(amount);
            state.current_call_amount += Uint128::from(amount);
            state.stage = new_stage.clone();
            state.community_cards = community.clone();

            OMAHA_STATE.save(deps.storage, &info.sender, &state)?;

            let mut global_state = STATE.load(deps.storage)?;
            global_state.locked_amount += sent;
            STATE.save(deps.storage, &global_state)?;

            Ok(Response::new()
                .add_attribute("action", "omaha_raise")
                .add_attribute("raise_amount", amount.to_string())
                .add_attribute("total_bet", state.player_total_bet.to_string())
                .add_attribute("stage", format!("{:?}", new_stage))
                .add_attribute("community_cards", format_cards(&community)))
        }

        // ── 跟注 ──────────────────────────────────────────────────────
        OmahaAction::Call => {
            let mut state = OMAHA_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if matches!(state.stage, OmahaStage::Showdown) {
                return Err(StdError::generic_err("Game is at Showdown, use Showdown action"));
            }

            // 需要补齐的差额
            let call_diff = state
                .current_call_amount
                .saturating_sub(state.player_total_bet)
                .u128();

            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if call_diff > 0 && sent < call_diff {
                return Err(StdError::generic_err(format!(
                    "Need to call at least {} uatom to match current bet",
                    call_diff
                )));
            }

            // 推进阶段
            let (new_stage, community) = advance_stage(&state);

            state.player_total_bet += Uint128::from(call_diff.max(sent));
            state.stage = new_stage.clone();
            state.community_cards = community.clone();

            OMAHA_STATE.save(deps.storage, &info.sender, &state)?;

            if sent > 0 {
                let mut global_state = STATE.load(deps.storage)?;
                global_state.locked_amount += sent;
                STATE.save(deps.storage, &global_state)?;
            }

            Ok(Response::new()
                .add_attribute("action", "omaha_call")
                .add_attribute("call_amount", call_diff.to_string())
                .add_attribute("total_bet", state.player_total_bet.to_string())
                .add_attribute("stage", format!("{:?}", new_stage))
                .add_attribute("community_cards", format_cards(&community)))
        }

        // ── 弃牌 ──────────────────────────────────────────────────────
        OmahaAction::Fold => {
            let mut state = OMAHA_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }

            state.finished = true;
            OMAHA_STATE.save(deps.storage, &info.sender, &state)?;

            // 玩家已下注金额归合约
            Ok(Response::new()
                .add_attribute("action", "omaha_fold")
                .add_attribute("result", "folded")
                .add_attribute("lost_amount", state.player_total_bet.to_string()))
        }

        // ── 摊牌结算 ───────────────────────────────────────────────────
        OmahaAction::Showdown => {
            let mut state = OMAHA_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }

            // 揭示全部 5 张公共牌
            let full_community: Vec<Card> = (8..13).map(|i| Card::from_id(state.deck[i])).collect();

            // 评估双方最佳手牌
            let player_rank = best_omaha_hand_rank(&state.player_hand, &full_community);
            let dealer_rank = best_omaha_hand_rank(&state.dealer_hand, &full_community);

            let player_hand_name = hand_rank_name(player_rank);
            let dealer_hand_name = hand_rank_name(dealer_rank);

            let total_bet = state.player_total_bet.u128();

            let mut global_state = STATE.load(deps.storage)?;
            let mut response = Response::new()
                .add_attribute("action", "omaha_showdown")
                .add_attribute("player_hand", format_cards(&state.player_hand))
                .add_attribute("dealer_hand", format_cards(&state.dealer_hand))
                .add_attribute("community_cards", format_cards(&full_community))
                .add_attribute("player_rank_name", player_hand_name)
                .add_attribute("dealer_rank_name", dealer_hand_name)
                .add_attribute("player_rank", player_rank.to_string())
                .add_attribute("dealer_rank", dealer_rank.to_string());

            if player_rank > dealer_rank {
                // 玩家赢：获得 2× 下注额
                let payout = total_bet * 2;
                global_state.locked_amount =
                    global_state.locked_amount.saturating_sub(payout);
                response = response
                    .add_attribute("result", "player_win")
                    .add_attribute("payout", payout.to_string())
                    .add_message(BankMsg::Send {
                        to_address: info.sender.to_string(),
                        amount: vec![Coin {
                            denom: "uatom".to_string(),
                            amount: Uint128::from(payout),
                        }],
                    });
            } else if dealer_rank > player_rank {
                // 庄家赢：玩家损失下注额（已留在合约中）
                response = response
                    .add_attribute("result", "dealer_win")
                    .add_attribute("payout", "0");
            } else {
                // 平局：退还下注额
                global_state.locked_amount =
                    global_state.locked_amount.saturating_sub(total_bet);
                response = response
                    .add_attribute("result", "tie")
                    .add_attribute("payout", total_bet.to_string())
                    .add_message(BankMsg::Send {
                        to_address: info.sender.to_string(),
                        amount: vec![Coin {
                            denom: "uatom".to_string(),
                            amount: Uint128::from(total_bet),
                        }],
                    });
            }

            state.finished = true;
            state.community_cards = full_community;
            OMAHA_STATE.save(deps.storage, &info.sender, &state)?;
            STATE.save(deps.storage, &global_state)?;

            Ok(response)
        }
    }
}

/// 根据当前阶段推进到下一阶段，并返回应揭示的公共牌列表
fn advance_stage(state: &OmahaState) -> (OmahaStage, Vec<Card>) {
    let deck = &state.deck;
    match state.stage {
        OmahaStage::PreFlop => {
            // 翻牌：揭示 3 张公共牌（deck[8..11]）
            let community = (8..11).map(|i| Card::from_id(deck[i])).collect();
            (OmahaStage::Flop, community)
        }
        OmahaStage::Flop => {
            // 转牌：揭示 4 张（deck[8..12]）
            let community = (8..12).map(|i| Card::from_id(deck[i])).collect();
            (OmahaStage::Turn, community)
        }
        OmahaStage::Turn => {
            // 河牌：揭示全部 5 张（deck[8..13]）
            let community = (8..13).map(|i| Card::from_id(deck[i])).collect();
            (OmahaStage::River, community)
        }
        OmahaStage::River | OmahaStage::Showdown => {
            // 已经揭示完毕，保持不变
            let community = state.community_cards.clone();
            (OmahaStage::Showdown, community)
        }
    }
}

/// 生成洗牌后的 52 张牌（card_id 0..=51）
fn shuffle_deck(info: &MessageInfo, env: &Env) -> Vec<u8> {
    // 用多个盐生成足够熵来 Fisher-Yates 洗牌
    let mut deck: Vec<u8> = (0u8..52).collect();
    // 为每张牌生成一个随机权重
    let mut weights: Vec<u32> = (0u8..52)
        .map(|i| {
            let salt = format!("omaha_deck_{}", i);
            utils::generate_random_number(info, env, salt.as_bytes())
        })
        .collect();

    // 按权重排序模拟洗牌（简单方案：稳定的伪随机排列）
    // Fisher-Yates 风格：用权重 XOR index 保证唯一性
    for i in (1..52usize).rev() {
        let j = (weights[i] as usize + i * 97) % (i + 1);
        deck.swap(i, j);
        weights.swap(i, j);
    }
    deck
}

/// 格式化 Card 列表为可读字符串
fn format_cards(cards: &[Card]) -> String {
    let parts: Vec<String> = cards
        .iter()
        .map(|c| {
            let rank_str = match c.rank {
                11 => "J".to_string(),
                12 => "Q".to_string(),
                13 => "K".to_string(),
                14 => "A".to_string(),
                n => n.to_string(),
            };
            let suit_str = match c.suit {
                omaha::Suit::Spades   => "♠",
                omaha::Suit::Hearts   => "♥",
                omaha::Suit::Diamonds => "♦",
                omaha::Suit::Clubs    => "♣",
            };
            format!("{}{}", rank_str, suit_str)
        })
        .collect();
    format!("[{}]", parts.join(","))
}

// ──────────────────────────────────────────────────────────────────────────────
// 德州扑克（Texas Hold'em）
//
// 流程：
//   1. Start        → 玩家下注底注，发 2 张手牌（庄家也发 2 张），进入 PreFlop
//   2. Raise/Call/Check → 在 PreFlop / Flop / Turn / River 阶段操作
//   3. AllIn        → 全押，直接进入 Showdown 自动揭示所有公共牌
//   4. Showdown     → 任意阶段直接摊牌结算
//   5. Fold         → 放弃本局，损失已押注金额
//
// 公共牌揭示节奏：
//   PreFlop  → Flop（3 张）→ Turn（+1 张）→ River（+1 张）→ Showdown
//
// 德州规则：从 2 张手牌 + 5 张公共牌中选最佳 5 张组合
// ──────────────────────────────────────────────────────────────────────────────
fn play_texas(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: TexasAction,
) -> StdResult<Response> {
    match action {
        // ── 开始游戏 ──────────────────────────────────────────────────
        TexasAction::Start => {
            // 检查是否已有进行中游戏
            if let Ok(existing) = TEXAS_STATE.load(deps.storage, &info.sender) {
                if !existing.finished {
                    return Err(StdError::generic_err(
                        "You already have an active Texas Hold'em game. Fold or Showdown first.",
                    ));
                }
            }

            let bet = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if bet < 100_000 || bet > 5_000_000 {
                return Err(StdError::generic_err(
                    "Initial bet must be between 100,000 and 5,000,000 uatom",
                ));
            }

            // 洗牌
            let deck = shuffle_texas_deck(&info, &env);

            // 发牌：玩家 2 张 (pos 0-1)，庄家 2 张 (pos 2-3)
            // 公共牌 5 张 (pos 4-8)，按阶段揭示
            let player_hand: Vec<texas::Card> =
                (0..2).map(|i| texas::Card::from_id(deck[i])).collect();
            let dealer_hand: Vec<texas::Card> =
                (2..4).map(|i| texas::Card::from_id(deck[i])).collect();

            let state = TexasState {
                player_hand: player_hand.clone(),
                dealer_hand,
                community_cards: vec![],
                player_total_bet: Uint128::from(bet),
                current_call_amount: Uint128::from(bet),
                stage: TexasStage::PreFlop,
                finished: false,
                all_in: false,
                deck: deck.clone(),
            };

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            let mut global_state = STATE.load(deps.storage)?;
            global_state.locked_amount += bet;
            STATE.save(deps.storage, &global_state)?;

            Ok(Response::new()
                .add_attribute("action", "texas_start")
                .add_attribute("stage", "PreFlop")
                .add_attribute("player_hand", format_texas_cards(&player_hand))
                .add_attribute("community_cards", "[]")
                .add_attribute("initial_bet", bet.to_string()))
        }

        // ── 加注 ──────────────────────────────────────────────────────
        TexasAction::Raise { amount } => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if state.all_in {
                return Err(StdError::generic_err("Already all-in, cannot raise"));
            }
            if matches!(state.stage, TexasStage::Showdown) {
                return Err(StdError::generic_err("Game is at Showdown, cannot raise"));
            }

            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if sent < amount || amount == 0 {
                return Err(StdError::generic_err(
                    "Must attach exactly the raise amount in uatom funds",
                ));
            }

            if amount < 50_000 {
                return Err(StdError::generic_err("Minimum raise is 50,000 uatom"));
            }

            let (new_stage, community) = advance_texas_stage(&state);

            state.player_total_bet += Uint128::from(amount);
            state.current_call_amount += Uint128::from(amount);
            state.stage = new_stage.clone();
            state.community_cards = community.clone();

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            let mut global_state = STATE.load(deps.storage)?;
            global_state.locked_amount += sent;
            STATE.save(deps.storage, &global_state)?;

            Ok(Response::new()
                .add_attribute("action", "texas_raise")
                .add_attribute("raise_amount", amount.to_string())
                .add_attribute("total_bet", state.player_total_bet.to_string())
                .add_attribute("stage", format!("{:?}", new_stage))
                .add_attribute("community_cards", format_texas_cards(&community)))
        }

        // ── 跟注 ──────────────────────────────────────────────────────
        TexasAction::Call => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if state.all_in {
                return Err(StdError::generic_err("Already all-in, cannot call"));
            }
            if matches!(state.stage, TexasStage::Showdown) {
                return Err(StdError::generic_err(
                    "Game is at Showdown, use Showdown action",
                ));
            }

            let call_diff = state
                .current_call_amount
                .saturating_sub(state.player_total_bet)
                .u128();

            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if call_diff > 0 && sent < call_diff {
                return Err(StdError::generic_err(format!(
                    "Need to call at least {} uatom to match current bet",
                    call_diff
                )));
            }

            let (new_stage, community) = advance_texas_stage(&state);

            state.player_total_bet += Uint128::from(call_diff.max(sent));
            state.stage = new_stage.clone();
            state.community_cards = community.clone();

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            if sent > 0 {
                let mut global_state = STATE.load(deps.storage)?;
                global_state.locked_amount += sent;
                STATE.save(deps.storage, &global_state)?;
            }

            Ok(Response::new()
                .add_attribute("action", "texas_call")
                .add_attribute("call_amount", call_diff.to_string())
                .add_attribute("total_bet", state.player_total_bet.to_string())
                .add_attribute("stage", format!("{:?}", new_stage))
                .add_attribute("community_cards", format_texas_cards(&community)))
        }

        // ── 过牌 ──────────────────────────────────────────────────────
        TexasAction::Check => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if state.all_in {
                return Err(StdError::generic_err("Already all-in, cannot check"));
            }
            if matches!(state.stage, TexasStage::Showdown) {
                return Err(StdError::generic_err(
                    "Game is at Showdown, use Showdown action",
                ));
            }

            // Check 仅在差额为 0 时允许（即无需补齐）
            let diff = state
                .current_call_amount
                .saturating_sub(state.player_total_bet)
                .u128();
            if diff > 0 {
                return Err(StdError::generic_err(
                    "Cannot check when you owe a call amount. Use Call or Raise instead.",
                ));
            }

            let (new_stage, community) = advance_texas_stage(&state);

            state.stage = new_stage.clone();
            state.community_cards = community.clone();

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            Ok(Response::new()
                .add_attribute("action", "texas_check")
                .add_attribute("total_bet", state.player_total_bet.to_string())
                .add_attribute("stage", format!("{:?}", new_stage))
                .add_attribute("community_cards", format_texas_cards(&community)))
        }

        // ── 全押 ──────────────────────────────────────────────────────
        TexasAction::AllIn { amount } => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }
            if state.all_in {
                return Err(StdError::generic_err("Already all-in"));
            }
            if matches!(state.stage, TexasStage::Showdown) {
                return Err(StdError::generic_err("Game is at Showdown, cannot all-in"));
            }

            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uatom")
                .map(|c| c.amount.u128())
                .unwrap_or(0);

            if sent < amount || amount == 0 {
                return Err(StdError::generic_err(
                    "Must attach exactly the all-in amount in uatom funds",
                ));
            }

            if amount < 100_000 {
                return Err(StdError::generic_err(
                    "All-in amount must be at least 100,000 uatom",
                ));
            }

            // 全押：揭示全部公共牌，进入 Showdown
            let full_community: Vec<texas::Card> =
                (4..9).map(|i| texas::Card::from_id(state.deck[i])).collect();

            state.player_total_bet += Uint128::from(amount);
            state.current_call_amount += Uint128::from(amount);
            state.all_in = true;
            state.community_cards = full_community.clone();
            state.stage = TexasStage::Showdown;

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            let mut global_state = STATE.load(deps.storage)?;
            global_state.locked_amount += sent;
            STATE.save(deps.storage, &global_state)?;

            // 自动进入 Showdown 结算
            settle_texas(deps, info, state)
        }

        // ── 弃牌 ──────────────────────────────────────────────────────
        TexasAction::Fold => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }

            state.finished = true;
            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            Ok(Response::new()
                .add_attribute("action", "texas_fold")
                .add_attribute("result", "folded")
                .add_attribute("lost_amount", state.player_total_bet.to_string()))
        }

        // ── 摊牌结算 ─────────────────────────────────────────────────
        TexasAction::Showdown => {
            let mut state = TEXAS_STATE.load(deps.storage, &info.sender)?;
            if state.finished {
                return Err(StdError::generic_err("Game already finished"));
            }

            // 揭示全部 5 张公共牌
            let full_community: Vec<texas::Card> =
                (4..9).map(|i| texas::Card::from_id(state.deck[i])).collect();
            state.community_cards = full_community;
            state.stage = TexasStage::Showdown;

            TEXAS_STATE.save(deps.storage, &info.sender, &state)?;

            settle_texas(deps, info, state)
        }
    }
}

/// 德州扑克结算逻辑
fn settle_texas(
    deps: DepsMut,
    info: MessageInfo,
    mut state: TexasState,
) -> StdResult<Response> {
    let full_community = &state.community_cards;

    // 评估双方最佳手牌
    let player_rank = best_texas_hand_rank(&state.player_hand, full_community);
    let dealer_rank = best_texas_hand_rank(&state.dealer_hand, full_community);

    let player_hand_name = texas::hand_rank_name(player_rank);
    let dealer_hand_name = texas::hand_rank_name(dealer_rank);

    let total_bet = state.player_total_bet.u128();

    let mut global_state = STATE.load(deps.storage)?;
    let mut response = Response::new()
        .add_attribute("action", "texas_showdown")
        .add_attribute("player_hand", format_texas_cards(&state.player_hand))
        .add_attribute("dealer_hand", format_texas_cards(&state.dealer_hand))
        .add_attribute(
            "community_cards",
            format_texas_cards(full_community),
        )
        .add_attribute("player_rank_name", player_hand_name)
        .add_attribute("dealer_rank_name", dealer_hand_name)
        .add_attribute("player_rank", player_rank.to_string())
        .add_attribute("dealer_rank", dealer_rank.to_string());

    if player_rank > dealer_rank {
        // 玩家赢：获得 2× 下注额
        let payout = total_bet * 2;
        global_state.locked_amount = global_state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "player_win")
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else if dealer_rank > player_rank {
        // 庄家赢
        response = response
            .add_attribute("result", "dealer_win")
            .add_attribute("payout", "0");
    } else {
        // 平局：退还下注额
        global_state.locked_amount = global_state.locked_amount.saturating_sub(total_bet);
        response = response
            .add_attribute("result", "tie")
            .add_attribute("payout", total_bet.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(total_bet),
                }],
            });
    }

    state.finished = true;
    TEXAS_STATE.save(deps.storage, &info.sender, &state)?;
    STATE.save(deps.storage, &global_state)?;

    Ok(response)
}

/// 德州扑克阶段推进
fn advance_texas_stage(state: &TexasState) -> (TexasStage, Vec<texas::Card>) {
    let deck = &state.deck;
    match state.stage {
        TexasStage::PreFlop => {
            // Flop: 揭示 3 张公共牌 (deck[4..7])
            let community = (4..7).map(|i| texas::Card::from_id(deck[i])).collect();
            (TexasStage::Flop, community)
        }
        TexasStage::Flop => {
            // Turn: 揭示 4 张 (deck[4..8])
            let community = (4..8).map(|i| texas::Card::from_id(deck[i])).collect();
            (TexasStage::Turn, community)
        }
        TexasStage::Turn => {
            // River: 揭示全部 5 张 (deck[4..9])
            let community = (4..9).map(|i| texas::Card::from_id(deck[i])).collect();
            (TexasStage::River, community)
        }
        TexasStage::River | TexasStage::Showdown => {
            let community = state.community_cards.clone();
            (TexasStage::Showdown, community)
        }
    }
}

/// 德州扑克洗牌（使用不同盐与奥马哈区分）
fn shuffle_texas_deck(info: &MessageInfo, env: &Env) -> Vec<u8> {
    let mut deck: Vec<u8> = (0u8..52).collect();
    let mut weights: Vec<u32> = (0u8..52)
        .map(|i| {
            let salt = format!("texas_deck_{}", i);
            utils::generate_random_number(info, env, salt.as_bytes())
        })
        .collect();

    for i in (1..52usize).rev() {
        let j = (weights[i] as usize + i * 97) % (i + 1);
        deck.swap(i, j);
        weights.swap(i, j);
    }
    deck
}

/// 格式化德州扑克 Card 列表为可读字符串
fn format_texas_cards(cards: &[texas::Card]) -> String {
    let parts: Vec<String> = cards
        .iter()
        .map(|c| {
            let rank_str = match c.rank {
                11 => "J".to_string(),
                12 => "Q".to_string(),
                13 => "K".to_string(),
                14 => "A".to_string(),
                n => n.to_string(),
            };
            let suit_str = match c.suit {
                texas::Suit::Spades   => "♠",
                texas::Suit::Hearts   => "♥",
                texas::Suit::Diamonds => "♦",
                texas::Suit::Clubs    => "♣",
            };
            format!("{}{}", rank_str, suit_str)
        })
        .collect();
    format!("[{}]", parts.join(","))
}

// ──────────────────────────────────────────────────────────────────────────────
// 三公（San Gong / Three Face Cards）
//
// 单局游戏：下注后双方各发 3 张牌，直接比较牌力
// 牌型：三公 > 混合九 > 普通点数 9 > ... > 0
// 赔率：三公赢 3×，混合九/普通赢 2×，平局退还本金
// ──────────────────────────────────────────────────────────────────────────────
fn play_sangong(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
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

    // 洗牌并发牌：玩家 3 张 + 庄家 3 张
    let deck = shuffle_sangong_deck(&info, &env);
    let player_cards: [SanGongCard; 3] = [
        SanGongCard::from_id(deck[0]),
        SanGongCard::from_id(deck[1]),
        SanGongCard::from_id(deck[2]),
    ];
    let dealer_cards: [SanGongCard; 3] = [
        SanGongCard::from_id(deck[3]),
        SanGongCard::from_id(deck[4]),
        SanGongCard::from_id(deck[5]),
    ];

    // 评估双方牌力
    let player_rank = evaluate_sangong_hand(&player_cards);
    let dealer_rank = evaluate_sangong_hand(&dealer_cards);

    let player_type_name = hand_type_name(&player_rank.hand_type);
    let dealer_type_name = hand_type_name(&dealer_rank.hand_type);

    let player_points = match &player_rank.hand_type {
        SanGongHandType::SanGong => 10,     // 显示用
        SanGongHandType::MixedNine => 9,
        SanGongHandType::Normal { points } => *points,
    };
    let dealer_points = match &dealer_rank.hand_type {
        SanGongHandType::SanGong => 10,
        SanGongHandType::MixedNine => 9,
        SanGongHandType::Normal { points } => *points,
    };

    let mut response = Response::new()
        .add_attribute("action", "play_sangong")
        .add_attribute("player_cards", format_sangong_cards(&player_cards))
        .add_attribute("dealer_cards", format_sangong_cards(&dealer_cards))
        .add_attribute("player_hand_type", player_type_name)
        .add_attribute("dealer_hand_type", dealer_type_name)
        .add_attribute("player_points", player_points.to_string())
        .add_attribute("dealer_points", dealer_points.to_string())
        .add_attribute("player_score", player_rank.score.to_string())
        .add_attribute("dealer_score", dealer_rank.score.to_string());

    if player_rank.score > dealer_rank.score {
        // 玩家赢
        let multiplier = sangong_payout_multiplier(&player_rank.hand_type);
        let payout = bet * multiplier;

        state.locked_amount = state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "win")
            .add_attribute("payout_multiplier", multiplier.to_string())
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else if dealer_rank.score > player_rank.score {
        // 庄家赢
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    } else {
        // 平局，退还本金
        state.locked_amount = state.locked_amount.saturating_sub(bet);
        response = response
            .add_attribute("result", "tie")
            .add_attribute("payout", bet.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(bet),
                }],
            });
    }

    STATE.save(deps.storage, &state)?;
    Ok(response)
}

/// 三公洗牌
fn shuffle_sangong_deck(info: &MessageInfo, env: &Env) -> Vec<u8> {
    let mut deck: Vec<u8> = (0u8..52).collect();
    let mut weights: Vec<u32> = (0u8..52)
        .map(|i| {
            let salt = format!("sangong_deck_{}", i);
            utils::generate_random_number(info, env, salt.as_bytes())
        })
        .collect();

    for i in (1..52usize).rev() {
        let j = (weights[i] as usize + i * 97) % (i + 1);
        deck.swap(i, j);
        weights.swap(i, j);
    }
    deck
}

/// 格式化三公牌面
fn format_sangong_cards(cards: &[SanGongCard]) -> String {
    let parts: Vec<String> = cards
        .iter()
        .map(|c| {
            let rank_str = match c.rank {
                1 => "A".to_string(),
                11 => "J".to_string(),
                12 => "Q".to_string(),
                13 => "K".to_string(),
                n => n.to_string(),
            };
            let suit_str = match c.suit {
                sangong::Suit::Spades   => "♠",
                sangong::Suit::Hearts   => "♥",
                sangong::Suit::Diamonds => "♦",
                sangong::Suit::Clubs    => "♣",
            };
            format!("{}{}", rank_str, suit_str)
        })
        .collect();
    format!("[{}]", parts.join(","))
}

// ──────────────────────────────────────────────────────────────────────────────
// 骰宝（Sic Bo）
//
// 单局游戏：下注后摇三颗骰子，根据投注类型判定输赢
// 投注类型：大/小、单/双、总和、三同号通选、指定三同号、双骰、单骰、两骰组合
// ──────────────────────────────────────────────────────────────────────────────
fn play_sicbo(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bet_type: SicBoBetType,
) -> StdResult<Response> {
    // 验证投注参数
    if let Err(e) = validate_bet(&bet_type) {
        return Err(StdError::generic_err(e));
    }

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

    // 摇三颗骰子（每颗 1-6）
    let die1 = (utils::generate_random_number(&info, &env, b"sicbo_die1") % 6 + 1) as u8;
    let die2 = (utils::generate_random_number(&info, &env, b"sicbo_die2") % 6 + 1) as u8;
    let die3 = (utils::generate_random_number(&info, &env, b"sicbo_die3") % 6 + 1) as u8;

    let result = SicBoResult::new(die1, die2, die3);

    // 计算赔付
    let (won, multiplier) = calculate_sicbo_payout(&bet_type, &result);

    let mut response = Response::new()
        .add_attribute("action", "play_sicbo")
        .add_attribute("die1", die1.to_string())
        .add_attribute("die2", die2.to_string())
        .add_attribute("die3", die3.to_string())
        .add_attribute("total", result.total.to_string())
        .add_attribute("is_triple", result.is_triple.to_string())
        .add_attribute("bet_type", format!("{:?}", bet_type));

    if won {
        let payout = bet * multiplier;

        state.locked_amount = state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "win")
            .add_attribute("payout_multiplier", multiplier.to_string())
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    STATE.save(deps.storage, &state)?;
    Ok(response)
}

// ──────────────────────────────────────────────────────────────────────────────
// 基诺（Keno）
//
// 单局游戏：玩家选 1-10 个号码（1-80），系统随机开出 20 个号码
// 命中越多赔率越高，选 10 中 10 最高赔 2000×
// ──────────────────────────────────────────────────────────────────────────────
fn play_keno(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    picks: Vec<u8>,
) -> StdResult<Response> {
    // 验证选号
    if let Err(e) = validate_picks(&picks) {
        return Err(StdError::generic_err(e));
    }

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

    // 从 1-80 中随机抽取 20 个不重复号码
    let drawn = draw_keno_numbers(&info, &env);

    // 计算命中
    let hits = calculate_hits(&picks, &drawn);
    let hit_count = hits.len() as u8;
    let pick_count = picks.len() as u8;

    // 计算赔率
    let multiplier = keno_payout_multiplier(pick_count, hit_count);

    let drawn_str: Vec<String> = drawn.iter().map(|n| n.to_string()).collect();
    let picks_str: Vec<String> = picks.iter().map(|n| n.to_string()).collect();
    let hits_str: Vec<String> = hits.iter().map(|n| n.to_string()).collect();

    let mut response = Response::new()
        .add_attribute("action", "play_keno")
        .add_attribute("picks", format!("[{}]", picks_str.join(",")))
        .add_attribute("drawn", format!("[{}]", drawn_str.join(",")))
        .add_attribute("hits", format!("[{}]", hits_str.join(",")))
        .add_attribute("pick_count", pick_count.to_string())
        .add_attribute("hit_count", hit_count.to_string());

    if multiplier > 0 {
        let payout = bet * multiplier;

        state.locked_amount = state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "win")
            .add_attribute("payout_multiplier", multiplier.to_string())
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    STATE.save(deps.storage, &state)?;
    Ok(response)
}

/// 从 1-80 中随机抽取 20 个不重复号码
fn draw_keno_numbers(info: &MessageInfo, env: &Env) -> Vec<u8> {
    // 生成 1-80 的号码池
    let mut pool: Vec<u8> = (1u8..=80).collect();

    // Fisher-Yates 洗牌（只需前 20 个位置）
    for i in 0..20usize {
        let salt = format!("keno_draw_{}", i);
        let rand = utils::generate_random_number(info, env, salt.as_bytes()) as usize;
        let j = i + (rand % (80 - i));
        pool.swap(i, j);
    }

    // 取前 20 个并排序（便于展示）
    let mut drawn: Vec<u8> = pool[..20].to_vec();
    drawn.sort_unstable();
    drawn
}

// ──────────────────────────────────────────────────────────────────────────────
// 刮刮乐（Scratch Card）
//
// 单局游戏：购买一张 3×3 的刮刮卡，刮开后检查 8 条中奖线
// 同行/同列/对角线三个相同符号即中奖，多线可叠加
// 符号：💎(50×) ⭐(20×) 🍀(10×) 🔔(5×) 🍒(3×) 🍋(2×)
// ──────────────────────────────────────────────────────────────────────────────
fn play_scratch_card(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    card_type: ScratchCardType,
) -> StdResult<Response> {
    // 检查下注金额
    let bet = info
        .funds
        .iter()
        .find(|c| c.denom == "uatom")
        .map(|c| c.amount.u128())
        .unwrap_or(0);

    let (min_bet, max_bet) = scratch_bet_range(&card_type);
    if bet < min_bet || bet > max_bet {
        return Err(StdError::generic_err(format!(
            "Bet must be between {} and {} uatom for {:?} card",
            min_bet, max_bet, card_type
        )));
    }

    // 更新合约锁仓金额
    let mut state = STATE.load(deps.storage)?;
    state.locked_amount += bet;

    // 生成 3×3 = 9 格符号
    let mut grid = [ScratchSymbol::Lemon; 9];
    for i in 0..9usize {
        let salt = format!("scratch_cell_{}", i);
        let rand = utils::generate_random_number(&info, &env, salt.as_bytes());
        grid[i] = ScratchSymbol::from_rand(rand);
    }

    // 评估中奖
    let (total_multiplier, winning_lines) = evaluate_scratch_card(&grid);

    // 构建格子展示
    let mut response = Response::new()
        .add_attribute("action", "play_scratch_card")
        .add_attribute("card_type", format!("{:?}", card_type))
        .add_attribute("bet_amount", bet.to_string());

    // 输出 3×3 格子
    for row in 0..3usize {
        for col in 0..3usize {
            let idx = row * 3 + col;
            let key = format!("cell_{}_{}", row + 1, col + 1);
            response = response.add_attribute(key, grid[idx].name());
        }
    }

    // 输出格子 emoji 行（便于阅读）
    for row in 0..3usize {
        let row_str: Vec<&str> = (0..3)
            .map(|col| grid[row * 3 + col].emoji())
            .collect();
        response = response.add_attribute(
            format!("row_{}", row + 1),
            row_str.join(" "),
        );
    }

    // 中奖线数
    let win_count = winning_lines.len() as u32;
    response = response.add_attribute("winning_lines", win_count.to_string());

    if total_multiplier > 0 {
        // 有中奖
        let payout = bet * total_multiplier;

        // 输出每条中奖线
        let win_desc: Vec<String> = winning_lines
            .iter()
            .map(|(line, sym)| format!("{}:{}", line, sym.name()))
            .collect();

        state.locked_amount = state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "win")
            .add_attribute("win_desc", win_desc.join(","))
            .add_attribute("total_multiplier", total_multiplier.to_string())
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else {
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    }

    STATE.save(deps.storage, &state)?;
    Ok(response)
}

// ──────────────────────────────────────────────────────────────────────────────
// 斗牛（Bull Bull / Niu Niu）
//
// 单局游戏：双方各发 5 张牌，比较牌型
// 特殊牌型：五小牛(8×) > 四炸(7×) > 五花牛(6×) > 牛牛(4×) > 牛9/8(3×) > 牛1-7(2×) > 没牛(2×)
// ──────────────────────────────────────────────────────────────────────────────
fn play_bullfight(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
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

    // 洗牌并发牌：玩家 5 张 + 庄家 5 张
    let deck = shuffle_bullfight_deck(&info, &env);
    let player_cards: [BullCard; 5] = [
        BullCard::from_id(deck[0]),
        BullCard::from_id(deck[1]),
        BullCard::from_id(deck[2]),
        BullCard::from_id(deck[3]),
        BullCard::from_id(deck[4]),
    ];
    let dealer_cards: [BullCard; 5] = [
        BullCard::from_id(deck[5]),
        BullCard::from_id(deck[6]),
        BullCard::from_id(deck[7]),
        BullCard::from_id(deck[8]),
        BullCard::from_id(deck[9]),
    ];

    // 评估双方牌力
    let player_rank = evaluate_bull_hand(&player_cards);
    let dealer_rank = evaluate_bull_hand(&dealer_cards);

    let player_type_name = bull_hand_type_name(&player_rank.hand_type);
    let dealer_type_name = bull_hand_type_name(&dealer_rank.hand_type);

    let mut response = Response::new()
        .add_attribute("action", "play_bullfight")
        .add_attribute("player_cards", format_bull_cards(&player_cards))
        .add_attribute("dealer_cards", format_bull_cards(&dealer_cards))
        .add_attribute("player_hand_type", player_type_name)
        .add_attribute("dealer_hand_type", dealer_type_name)
        .add_attribute("player_score", player_rank.score.to_string())
        .add_attribute("dealer_score", dealer_rank.score.to_string());

    if player_rank.score > dealer_rank.score {
        // 玩家赢
        let multiplier = bull_payout_multiplier(&player_rank.hand_type);
        let payout = bet * multiplier;

        state.locked_amount = state.locked_amount.saturating_sub(payout);
        response = response
            .add_attribute("result", "win")
            .add_attribute("payout_multiplier", multiplier.to_string())
            .add_attribute("payout", payout.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(payout),
                }],
            });
    } else if dealer_rank.score > player_rank.score {
        // 庄家赢
        response = response
            .add_attribute("result", "lose")
            .add_attribute("payout", "0");
    } else {
        // 平局，退还本金
        state.locked_amount = state.locked_amount.saturating_sub(bet);
        response = response
            .add_attribute("result", "tie")
            .add_attribute("payout", bet.to_string())
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![Coin {
                    denom: "uatom".to_string(),
                    amount: Uint128::from(bet),
                }],
            });
    }

    STATE.save(deps.storage, &state)?;
    Ok(response)
}

/// 斗牛洗牌
fn shuffle_bullfight_deck(info: &MessageInfo, env: &Env) -> Vec<u8> {
    let mut deck: Vec<u8> = (0u8..52).collect();
    let mut weights: Vec<u32> = (0u8..52)
        .map(|i| {
            let salt = format!("bullfight_deck_{}", i);
            utils::generate_random_number(info, env, salt.as_bytes())
        })
        .collect();

    for i in (1..52usize).rev() {
        let j = (weights[i] as usize + i * 97) % (i + 1);
        deck.swap(i, j);
        weights.swap(i, j);
    }
    deck
}

/// 格式化斗牛牌面
fn format_bull_cards(cards: &[BullCard]) -> String {
    let parts: Vec<String> = cards
        .iter()
        .map(|c| {
            let rank_str = match c.rank {
                1 => "A".to_string(),
                11 => "J".to_string(),
                12 => "Q".to_string(),
                13 => "K".to_string(),
                n => n.to_string(),
            };
            let suit_str = match c.suit {
                bullfight::Suit::Spades   => "♠",
                bullfight::Suit::Hearts   => "♥",
                bullfight::Suit::Diamonds => "♦",
                bullfight::Suit::Clubs    => "♣",
            };
            format!("{}{}", rank_str, suit_str)
        })
        .collect();
    format!("[{}]", parts.join(","))
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
    pub fn test_play_slot_basic() {
        let mut deps = mock_dependencies();

        // 初始化合约
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Basic 模式下注
        let user_info = mock_info("user", &coins(100_000, "uatom"));
        let env = mock_env();
        let res = play_slot(deps.as_mut(), env.clone(), user_info.clone(), SlotMode::Basic).unwrap();

        let attrs = &res.attributes;

        // 检查 mode 属性
        let mode_attr = attrs.iter().find(|a| a.key == "mode").expect("mode missing");
        assert_eq!(mode_attr.value, "Basic");

        // 检查下注金额
        let bet_attr = attrs.iter().find(|a| a.key == "bet_amount").expect("bet_amount missing");
        assert_eq!(bet_attr.value, "100000");

        // 检查三个轮盘符号存在
        assert!(attrs.iter().any(|a| a.key == "reel1"), "reel1 missing");
        assert!(attrs.iter().any(|a| a.key == "reel2"), "reel2 missing");
        assert!(attrs.iter().any(|a| a.key == "reel3"), "reel3 missing");

        // 检查 win_desc
        assert!(attrs.iter().any(|a| a.key == "win_desc"), "win_desc missing");

        // 应该有 result 或 payout_multiplier
        let has_result = attrs.iter().any(|a| a.key == "result");
        let has_payout = attrs.iter().any(|a| a.key == "payout_multiplier");
        assert!(has_result || has_payout, "expected result or payout_multiplier");

        // 投注金额下限检查
        let too_small = play_slot(
            deps.as_mut(), env.clone(),
            mock_info("user", &coins(50_000, "uatom")),
            SlotMode::Basic,
        );
        assert!(too_small.is_err(), "should reject bet below minimum");
    }

    #[test]
    pub fn test_play_slot_advanced() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Advanced 模式下注
        let user_info = mock_info("user", &coins(500_000, "uatom"));
        let env = mock_env();
        let res = play_slot(deps.as_mut(), env.clone(), user_info.clone(), SlotMode::Advanced).unwrap();

        let attrs = &res.attributes;

        // 检查 mode 属性
        let mode_attr = attrs.iter().find(|a| a.key == "mode").expect("mode missing");
        assert_eq!(mode_attr.value, "Advanced");

        // 检查 5 列 × 3 行 = 15 个格子都有输出
        for col in 1..=5usize {
            for row in 1..=3usize {
                let key = format!("reel{}_{}", col, row);
                assert!(
                    attrs.iter().any(|a| a.key == key),
                    "missing grid cell {}",
                    key
                );
            }
        }

        // 检查 win_desc
        assert!(attrs.iter().any(|a| a.key == "win_desc"), "win_desc missing");

        // 应有 result 或 payout_multiplier
        let has_result = attrs.iter().any(|a| a.key == "result");
        let has_payout = attrs.iter().any(|a| a.key == "payout_multiplier");
        assert!(has_result || has_payout, "expected result or payout_multiplier");

        // Advanced 模式最低下注 200_000，低于时应报错
        let too_small = play_slot(
            deps.as_mut(), env.clone(),
            mock_info("user", &coins(100_000, "uatom")),
            SlotMode::Advanced,
        );
        assert!(too_small.is_err(), "should reject bet below Advanced minimum");
    }

    #[test]
    pub fn test_play_slot_win_payout() {
        // 测试赢时支付金额正确（通过 execute 入口）
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user = "player1";
        let bet = 1_000_000u128;
        let user_info = mock_info(user, &coins(bet, "uatom"));
        let env = mock_env();

        let res = execute(
            deps.as_mut(), env, user_info,
            ExecuteMsg::PlaySlot { mode: SlotMode::Basic },
        ).unwrap();

        let has_result = res.attributes.iter().any(|a| a.key == "result");
        let has_payout = res.attributes.iter().any(|a| a.key == "payout_multiplier");
        assert!(has_result || has_payout);

        if has_payout {
            // 如果赢了，支付消息必须存在
            assert_eq!(res.messages.len(), 1);
            let payout_mult: u128 = res
                .attributes
                .iter()
                .find(|a| a.key == "payout_multiplier")
                .unwrap()
                .value
                .parse()
                .unwrap();
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * payout_mult, "uatom"),
                }
                .into()
            );
        } else {
            // 如果输了，没有支付消息
            assert_eq!(res.messages.len(), 0);
        }
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

    // ──────────────────────────────────────────────────────────────────────
    // Mega Slot 测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_slot_mega() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Mega 模式最低投注 500_000
        let user_info = mock_info("user", &coins(500_000, "uatom"));
        let res = play_slot(deps.as_mut(), mock_env(), user_info, SlotMode::Mega).unwrap();
        let attrs = &res.attributes;

        // mode 属性
        let mode_attr = attrs.iter().find(|a| a.key == "mode").expect("mode missing");
        assert_eq!(mode_attr.value, "Mega");

        // 6 列 × 4 行 = 24 格
        for col in 1..=6usize {
            for row in 1..=4usize {
                let key = format!("reel{}_{}", col, row);
                assert!(attrs.iter().any(|a| a.key == key), "missing {}", key);
            }
        }

        // 必须包含 free_spin 和 jackpot 属性
        assert!(attrs.iter().any(|a| a.key == "free_spin"), "free_spin missing");
        assert!(attrs.iter().any(|a| a.key == "jackpot"), "jackpot missing");
        assert!(attrs.iter().any(|a| a.key == "win_desc"), "win_desc missing");

        // 低于最低投注应报错
        let too_small = play_slot(
            deps.as_mut(), mock_env(),
            mock_info("user", &coins(100_000, "uatom")),
            SlotMode::Mega,
        );
        assert!(too_small.is_err(), "should reject bet below Mega minimum");
    }

    #[test]
    fn test_play_slot_mega_payout() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let user = "player1";
        let bet = 1_000_000u128;
        let res = execute(
            deps.as_mut(), mock_env(),
            mock_info(user, &coins(bet, "uatom")),
            ExecuteMsg::PlaySlot { mode: SlotMode::Mega },
        ).unwrap();

        let has_result = res.attributes.iter().any(|a| a.key == "result");
        let has_payout = res.attributes.iter().any(|a| a.key == "payout_multiplier");
        assert!(has_result || has_payout, "expected result or payout_multiplier");

        if has_payout {
            assert_eq!(res.messages.len(), 1);
            let payout_mult: u128 = res
                .attributes
                .iter()
                .find(|a| a.key == "payout_multiplier")
                .unwrap()
                .value
                .parse()
                .unwrap();
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * payout_mult, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // 奥马哈扑克测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_omaha_full_flow() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "omaha_player";
        let bet = 500_000u128;

        // ── Step 1: Start ──────────────────────────────────────
        let start_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(bet, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Start },
        ).unwrap();

        let stage = start_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(stage.value, "PreFlop");

        let player_hand_attr = start_res.attributes.iter().find(|a| a.key == "player_hand").unwrap();
        assert!(!player_hand_attr.value.is_empty(), "player_hand should not be empty");

        // ── Step 2: Query state ────────────────────────────────
        let bin = query(
            deps.as_ref(), mock_env(),
            QueryMsg::GetOmahaState { address: player.to_string() },
        ).unwrap();
        let state_resp: OmahaStateResponse = from_json(&bin).unwrap();
        assert_eq!(state_resp.player_hand.len(), 4, "player should have 4 cards");
        assert_eq!(state_resp.dealer_hand.len(), 0, "dealer hand hidden before showdown");
        assert_eq!(state_resp.community_cards.len(), 0, "no community cards at PreFlop");
        assert!(!state_resp.finished);

        // ── Step 3: Raise (PreFlop → Flop) ───────────────────
        let raise_amount = 200_000u128;
        let raise_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(raise_amount, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Raise { amount: raise_amount } },
        ).unwrap();

        let raise_stage = raise_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(raise_stage.value, "Flop");

        let community_attr = raise_res.attributes.iter().find(|a| a.key == "community_cards").unwrap();
        assert!(community_attr.value.contains(","), "flop should have 3 cards");

        // ── Step 4: Call (Flop → Turn) ────────────────────────
        let call_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Call },
        ).unwrap();
        let call_stage = call_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(call_stage.value, "Turn");

        // ── Step 5: Showdown ──────────────────────────────────
        let showdown_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Showdown },
        ).unwrap();

        let result = showdown_res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        assert!(
            result.value == "player_win" || result.value == "dealer_win" || result.value == "tie",
            "unexpected result: {}",
            result.value
        );

        // showdown 后庄家手牌应可见
        let bin2 = query(
            deps.as_ref(), mock_env(),
            QueryMsg::GetOmahaState { address: player.to_string() },
        ).unwrap();
        let final_state: OmahaStateResponse = from_json(&bin2).unwrap();
        assert!(final_state.finished);
        assert_eq!(final_state.dealer_hand.len(), 4, "dealer hand visible after showdown");
        assert_eq!(final_state.community_cards.len(), 5, "all 5 community cards after showdown");
    }

    #[test]
    fn test_omaha_fold() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "omaha_fold_player";
        let bet = 300_000u128;

        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(bet, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Start },
        ).unwrap();

        // 弃牌
        let fold_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Fold },
        ).unwrap();

        let result = fold_res.attributes.iter().find(|a| a.key == "result").unwrap();
        assert_eq!(result.value, "folded");

        // 弃牌后游戏结束，不能再操作
        let err = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Showdown },
        );
        assert!(err.is_err(), "should not be able to showdown after fold");
    }

    #[test]
    fn test_omaha_no_duplicate_game() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "omaha_dup_player";

        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Start },
        ).unwrap();

        // 重复开始应报错
        let err = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Start },
        );
        assert!(err.is_err(), "should not allow duplicate active game");
    }

    #[test]
    fn test_omaha_raise_then_showdown() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "omaha_raise_player";

        // Start
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(500_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Start },
        ).unwrap();

        // Raise 1: PreFlop → Flop
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Raise { amount: 100_000 } },
        ).unwrap();

        // Raise 2: Flop → Turn
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(200_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Raise { amount: 200_000 } },
        ).unwrap();

        // Raise 3: Turn → River
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(300_000, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Raise { amount: 300_000 } },
        ).unwrap();

        // Showdown
        let sd = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayOmaha { action: OmahaAction::Showdown },
        ).unwrap();

        let result = sd.attributes.iter().find(|a| a.key == "result").unwrap();
        assert!(
            ["player_win", "dealer_win", "tie"].contains(&result.value.as_str()),
            "unexpected result: {}",
            result.value
        );

        // 验证 player_rank_name 和 dealer_rank_name 存在
        assert!(sd.attributes.iter().any(|a| a.key == "player_rank_name"));
        assert!(sd.attributes.iter().any(|a| a.key == "dealer_rank_name"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // 德州扑克（Texas Hold'em）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_texas_full_flow() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_player";
        let bet = 500_000u128;

        // ── Step 1: Start ──────────────────────────────────────
        let start_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(bet, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        let stage = start_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(stage.value, "PreFlop");

        let player_hand_attr = start_res.attributes.iter().find(|a| a.key == "player_hand").unwrap();
        assert!(!player_hand_attr.value.is_empty(), "player_hand should not be empty");

        // ── Step 2: Query state ────────────────────────────────
        let bin = query(
            deps.as_ref(), mock_env(),
            QueryMsg::GetTexasState { address: player.to_string() },
        ).unwrap();
        let state_resp: TexasStateResponse = from_json(&bin).unwrap();
        assert_eq!(state_resp.player_hand.len(), 2, "player should have 2 cards");
        assert_eq!(state_resp.dealer_hand.len(), 0, "dealer hand hidden before showdown");
        assert_eq!(state_resp.community_cards.len(), 0, "no community cards at PreFlop");
        assert!(!state_resp.finished);
        assert!(!state_resp.all_in);

        // ── Step 3: Raise (PreFlop → Flop) ───────────────────
        let raise_amount = 200_000u128;
        let raise_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(raise_amount, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Raise { amount: raise_amount } },
        ).unwrap();

        let raise_stage = raise_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(raise_stage.value, "Flop");

        let community_attr = raise_res.attributes.iter().find(|a| a.key == "community_cards").unwrap();
        assert!(community_attr.value.contains(","), "flop should have 3 cards");

        // ── Step 4: Call (Flop → Turn) ────────────────────────
        let call_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Call },
        ).unwrap();
        let call_stage = call_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(call_stage.value, "Turn");

        // ── Step 5: Showdown ──────────────────────────────────
        let showdown_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Showdown },
        ).unwrap();

        let result = showdown_res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        assert!(
            result.value == "player_win" || result.value == "dealer_win" || result.value == "tie",
            "unexpected result: {}",
            result.value
        );

        // Showdown 后庄家手牌应可见
        let bin2 = query(
            deps.as_ref(), mock_env(),
            QueryMsg::GetTexasState { address: player.to_string() },
        ).unwrap();
        let final_state: TexasStateResponse = from_json(&bin2).unwrap();
        assert!(final_state.finished);
        assert_eq!(final_state.dealer_hand.len(), 2, "dealer hand visible after showdown");
        assert_eq!(final_state.community_cards.len(), 5, "all 5 community cards after showdown");
    }

    #[test]
    fn test_texas_fold() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_fold_player";
        let bet = 300_000u128;

        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(bet, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        // 弃牌
        let fold_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Fold },
        ).unwrap();

        let result = fold_res.attributes.iter().find(|a| a.key == "result").unwrap();
        assert_eq!(result.value, "folded");

        // 弃牌后不能再操作
        let err = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Showdown },
        );
        assert!(err.is_err(), "should not be able to showdown after fold");
    }

    #[test]
    fn test_texas_no_duplicate_game() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_dup_player";

        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        // 重复开始应报错
        let err = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        );
        assert!(err.is_err(), "should not allow duplicate active game");
    }

    #[test]
    fn test_texas_check_advance() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_check_player";

        // Start
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(500_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        // Check (PreFlop → Flop) — 差额为 0，可以过牌
        let check_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Check },
        ).unwrap();
        let check_stage = check_res.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(check_stage.value, "Flop");

        // Check (Flop → Turn)
        let check2 = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Check },
        ).unwrap();
        let check2_stage = check2.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(check2_stage.value, "Turn");

        // Check (Turn → River)
        let check3 = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Check },
        ).unwrap();
        let check3_stage = check3.attributes.iter().find(|a| a.key == "stage").unwrap();
        assert_eq!(check3_stage.value, "River");

        // Showdown
        let sd = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Showdown },
        ).unwrap();
        let result = sd.attributes.iter().find(|a| a.key == "result").unwrap();
        assert!(
            ["player_win", "dealer_win", "tie"].contains(&result.value.as_str()),
            "unexpected result: {}",
            result.value
        );
    }

    #[test]
    fn test_texas_all_in() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_allin_player";

        // Start
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(500_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        // All-In 直接进入 Showdown
        let allin_res = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(2_000_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::AllIn { amount: 2_000_000 } },
        ).unwrap();

        let result = allin_res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        assert!(
            ["player_win", "dealer_win", "tie"].contains(&result.value.as_str()),
            "unexpected result: {}",
            result.value
        );

        // 验证 rank_name 属性存在
        assert!(allin_res.attributes.iter().any(|a| a.key == "player_rank_name"));
        assert!(allin_res.attributes.iter().any(|a| a.key == "dealer_rank_name"));

        // 验证游戏已结束
        let bin = query(
            deps.as_ref(), mock_env(),
            QueryMsg::GetTexasState { address: player.to_string() },
        ).unwrap();
        let final_state: TexasStateResponse = from_json(&bin).unwrap();
        assert!(final_state.finished);
        assert!(final_state.all_in);
        assert_eq!(final_state.community_cards.len(), 5);
        assert_eq!(final_state.dealer_hand.len(), 2);
    }

    #[test]
    fn test_texas_raise_then_showdown() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let player = "texas_raise_player";

        // Start
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(500_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Start },
        ).unwrap();

        // Raise 1: PreFlop → Flop
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(100_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Raise { amount: 100_000 } },
        ).unwrap();

        // Raise 2: Flop → Turn
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(200_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Raise { amount: 200_000 } },
        ).unwrap();

        // Raise 3: Turn → River
        execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(300_000, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Raise { amount: 300_000 } },
        ).unwrap();

        // Showdown
        let sd = execute(
            deps.as_mut(), mock_env(),
            mock_info(player, &coins(0, "uatom")),
            ExecuteMsg::PlayTexas { action: TexasAction::Showdown },
        ).unwrap();

        let result = sd.attributes.iter().find(|a| a.key == "result").unwrap();
        assert!(
            ["player_win", "dealer_win", "tie"].contains(&result.value.as_str()),
            "unexpected result: {}",
            result.value
        );

        assert!(sd.attributes.iter().any(|a| a.key == "player_rank_name"));
        assert!(sd.attributes.iter().any(|a| a.key == "dealer_rank_name"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // 三公（San Gong）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_sangong() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sangong_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info.clone(),
            ExecuteMsg::PlaySanGong {},
        )
        .unwrap();

        // 检查基本属性
        let action = res
            .attributes
            .iter()
            .find(|a| a.key == "action")
            .expect("action missing");
        assert_eq!(action.value, "play_sangong");

        assert!(
            res.attributes.iter().any(|a| a.key == "player_cards"),
            "player_cards missing"
        );
        assert!(
            res.attributes.iter().any(|a| a.key == "dealer_cards"),
            "dealer_cards missing"
        );
        assert!(
            res.attributes.iter().any(|a| a.key == "player_hand_type"),
            "player_hand_type missing"
        );
        assert!(
            res.attributes.iter().any(|a| a.key == "dealer_hand_type"),
            "dealer_hand_type missing"
        );
        assert!(
            res.attributes.iter().any(|a| a.key == "player_points"),
            "player_points missing"
        );
        assert!(
            res.attributes.iter().any(|a| a.key == "dealer_points"),
            "dealer_points missing"
        );

        let result = res
            .attributes
            .iter()
            .find(|a| a.key == "result")
            .expect("result missing");

        if result.value == "win" {
            assert_eq!(res.messages.len(), 1, "should have payout message on win");
            // 检查 payout_multiplier 属性存在
            let mult = res
                .attributes
                .iter()
                .find(|a| a.key == "payout_multiplier")
                .expect("payout_multiplier missing");
            let mult_val: u128 = mult.value.parse().unwrap();
            assert!(
                mult_val >= 2 && mult_val <= 3,
                "multiplier should be 2 or 3, got {}",
                mult_val
            );
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(1_000_000 * mult_val, "uatom"),
                }
                .into()
            );
        } else if result.value == "tie" {
            // 平局退还本金
            assert_eq!(res.messages.len(), 1, "should refund on tie");
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(1_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(result.value, "lose");
            assert_eq!(res.messages.len(), 0, "should have no payout on loss");
        }
    }

    #[test]
    fn test_sangong_bet_limits() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        // 低于最小下注
        let too_small = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(50_000, "uatom")),
            ExecuteMsg::PlaySanGong {},
        );
        assert!(too_small.is_err(), "should reject bet below 100,000");

        // 高于最大下注
        let too_big = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(20_000_000, "uatom")),
            ExecuteMsg::PlaySanGong {},
        );
        assert!(too_big.is_err(), "should reject bet above 10,000,000");
    }

    #[test]
    fn test_sangong_hand_evaluation() {
        use crate::sangong::{evaluate_sangong_hand, SanGongCard, SanGongHandType, Suit};

        // 三公：三张全是公牌 (K, Q, J)
        let san_gong = [
            SanGongCard { rank: 13, suit: Suit::Spades },
            SanGongCard { rank: 12, suit: Suit::Hearts },
            SanGongCard { rank: 11, suit: Suit::Diamonds },
        ];
        let rank = evaluate_sangong_hand(&san_gong);
        assert_eq!(rank.hand_type, SanGongHandType::SanGong);
        assert!(rank.score >= 30_000, "san gong should have highest score");

        // 混合九：含公牌且点数为 9（K + 4 + 5 = 0+4+5 = 9）
        let mixed_nine = [
            SanGongCard { rank: 13, suit: Suit::Spades },
            SanGongCard { rank: 4, suit: Suit::Hearts },
            SanGongCard { rank: 5, suit: Suit::Diamonds },
        ];
        let rank2 = evaluate_sangong_hand(&mixed_nine);
        assert_eq!(rank2.hand_type, SanGongHandType::MixedNine);
        assert!(rank2.score >= 20_000 && rank2.score < 30_000);

        // 普通 7 点 (3 + 2 + 2 = 7)
        let normal = [
            SanGongCard { rank: 3, suit: Suit::Spades },
            SanGongCard { rank: 2, suit: Suit::Hearts },
            SanGongCard { rank: 2, suit: Suit::Diamonds },
        ];
        let rank3 = evaluate_sangong_hand(&normal);
        assert_eq!(rank3.hand_type, SanGongHandType::Normal { points: 7 });
        assert!(rank3.score < 20_000);

        // 三公 > 混合九 > 普通
        assert!(rank.score > rank2.score);
        assert!(rank2.score > rank3.score);
    }

    // ──────────────────────────────────────────────────────────────────────
    // 骰宝（Sic Bo）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_sicbo_big() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Big,
            },
        )
        .unwrap();

        // 检查基本属性
        assert_eq!(
            res.attributes.iter().find(|a| a.key == "action").unwrap().value,
            "play_sicbo"
        );
        assert!(res.attributes.iter().any(|a| a.key == "die1"));
        assert!(res.attributes.iter().any(|a| a.key == "die2"));
        assert!(res.attributes.iter().any(|a| a.key == "die3"));
        assert!(res.attributes.iter().any(|a| a.key == "total"));
        assert!(res.attributes.iter().any(|a| a.key == "is_triple"));

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        let total: u8 = res.attributes.iter().find(|a| a.key == "total").unwrap().value.parse().unwrap();
        let is_triple: bool = res.attributes.iter().find(|a| a.key == "is_triple").unwrap().value.parse().unwrap();

        if result.value == "win" {
            assert!(total >= 11 && total <= 17 && !is_triple, "Big win requires total 11-17, no triple");
            assert_eq!(res.messages.len(), 1);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(2_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_sicbo_small() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_small_player";
        let user_info = mock_info(user, &coins(500_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Small,
            },
        )
        .unwrap();

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        let total: u8 = res.attributes.iter().find(|a| a.key == "total").unwrap().value.parse().unwrap();
        let is_triple: bool = res.attributes.iter().find(|a| a.key == "is_triple").unwrap().value.parse().unwrap();

        if result.value == "win" {
            assert!(total >= 4 && total <= 10 && !is_triple, "Small win requires total 4-10, no triple");
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(1_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_sicbo_total() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_total_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Total { value: 10 },
            },
        )
        .unwrap();

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        let total: u8 = res.attributes.iter().find(|a| a.key == "total").unwrap().value.parse().unwrap();

        if result.value == "win" {
            assert_eq!(total, 10);
            // 总和 10 赔率 7×
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(7_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_sicbo_any_triple() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_triple_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::AnyTriple,
            },
        )
        .unwrap();

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");
        let is_triple: bool = res.attributes.iter().find(|a| a.key == "is_triple").unwrap().value.parse().unwrap();

        if result.value == "win" {
            assert!(is_triple, "AnyTriple win requires all three dice the same");
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(31_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_sicbo_single_die() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_single_player";
        let bet = 1_000_000u128;
        let user_info = mock_info(user, &coins(bet, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::SingleDie { number: 3 },
            },
        )
        .unwrap();

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");

        if result.value == "win" {
            let payout_mult: u128 = res.attributes.iter()
                .find(|a| a.key == "payout_multiplier")
                .unwrap()
                .value.parse().unwrap();
            assert!(payout_mult >= 2 && payout_mult <= 4, "single die multiplier 2-4");
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * payout_mult, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_sicbo_combo() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "sicbo_combo_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Combo { first: 1, second: 6 },
            },
        )
        .unwrap();

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");

        if result.value == "win" {
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(7_000_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_sicbo_bet_validation() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        // 总和超出范围
        let err = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Total { value: 20 },
            },
        );
        assert!(err.is_err(), "should reject total > 17");

        let err2 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Total { value: 2 },
            },
        );
        assert!(err2.is_err(), "should reject total < 4");

        // 骰子点数超出范围
        let err3 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::SingleDie { number: 7 },
            },
        );
        assert!(err3.is_err(), "should reject die number > 6");

        // 两骰组合相同数字
        let err4 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Combo { first: 3, second: 3 },
            },
        );
        assert!(err4.is_err(), "should reject combo with same numbers");

        // 下注金额过小
        let err5 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(50_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Big,
            },
        );
        assert!(err5.is_err(), "should reject bet below 100,000");

        // 下注金额过大
        let err6 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(20_000_000, "uatom")),
            ExecuteMsg::PlaySicBo {
                bet_type: SicBoBetType::Big,
            },
        );
        assert!(err6.is_err(), "should reject bet above 10,000,000");
    }

    #[test]
    fn test_sicbo_payout_logic() {
        use crate::sicbo::{calculate_sicbo_payout, SicBoBetType, SicBoResult};

        // 大：总和 12，非三同号 → 赢
        let r = SicBoResult::new(4, 4, 4); // triple 12
        let (won, _) = calculate_sicbo_payout(&SicBoBetType::Big, &r);
        assert!(!won, "triple should lose Big bet");

        let r2 = SicBoResult::new(3, 4, 5); // total 12, no triple
        let (won2, mult2) = calculate_sicbo_payout(&SicBoBetType::Big, &r2);
        assert!(won2, "total 12 no triple should win Big");
        assert_eq!(mult2, 2);

        // 小：总和 8，非三同号 → 赢
        let r3 = SicBoResult::new(1, 3, 4); // total 8
        let (won3, mult3) = calculate_sicbo_payout(&SicBoBetType::Small, &r3);
        assert!(won3);
        assert_eq!(mult3, 2);

        // 单双
        let r4 = SicBoResult::new(1, 2, 4); // total 7 (odd)
        let (odd_won, _) = calculate_sicbo_payout(&SicBoBetType::Odd, &r4);
        assert!(odd_won);
        let (even_won, _) = calculate_sicbo_payout(&SicBoBetType::Even, &r4);
        assert!(!even_won);

        // 指定三同号
        let r5 = SicBoResult::new(6, 6, 6);
        let (triple_won, triple_mult) = calculate_sicbo_payout(
            &SicBoBetType::SpecificTriple { number: 6 },
            &r5,
        );
        assert!(triple_won);
        assert_eq!(triple_mult, 181);

        let (wrong_triple, _) = calculate_sicbo_payout(
            &SicBoBetType::SpecificTriple { number: 5 },
            &r5,
        );
        assert!(!wrong_triple);

        // 双骰
        let r6 = SicBoResult::new(3, 3, 5);
        let (double_won, double_mult) = calculate_sicbo_payout(
            &SicBoBetType::DoubleBet { number: 3 },
            &r6,
        );
        assert!(double_won);
        assert_eq!(double_mult, 12);

        // 单骰：出现 1 次
        let r7 = SicBoResult::new(2, 4, 6);
        let (single_won, single_mult) = calculate_sicbo_payout(
            &SicBoBetType::SingleDie { number: 4 },
            &r7,
        );
        assert!(single_won);
        assert_eq!(single_mult, 2);

        // 单骰：出现 2 次
        let r8 = SicBoResult::new(4, 4, 6);
        let (s2_won, s2_mult) = calculate_sicbo_payout(
            &SicBoBetType::SingleDie { number: 4 },
            &r8,
        );
        assert!(s2_won);
        assert_eq!(s2_mult, 3);

        // 两骰组合
        let r9 = SicBoResult::new(1, 3, 6);
        let (combo_won, combo_mult) = calculate_sicbo_payout(
            &SicBoBetType::Combo { first: 1, second: 6 },
            &r9,
        );
        assert!(combo_won);
        assert_eq!(combo_mult, 7);

        let (combo_fail, _) = calculate_sicbo_payout(
            &SicBoBetType::Combo { first: 2, second: 5 },
            &r9,
        );
        assert!(!combo_fail);
    }

    // ──────────────────────────────────────────────────────────────────────
    // 基诺（Keno）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_keno_basic() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "keno_player";
        let bet = 1_000_000u128;
        let user_info = mock_info(user, &coins(bet, "uatom"));

        // 选 5 个号码
        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayKeno {
                picks: vec![3, 17, 28, 42, 65],
            },
        )
        .unwrap();

        // 检查基本属性
        assert_eq!(
            res.attributes.iter().find(|a| a.key == "action").unwrap().value,
            "play_keno"
        );
        assert!(res.attributes.iter().any(|a| a.key == "picks"));
        assert!(res.attributes.iter().any(|a| a.key == "drawn"));
        assert!(res.attributes.iter().any(|a| a.key == "hits"));
        assert!(res.attributes.iter().any(|a| a.key == "pick_count"));
        assert!(res.attributes.iter().any(|a| a.key == "hit_count"));

        let pick_count: u8 = res
            .attributes.iter().find(|a| a.key == "pick_count").unwrap()
            .value.parse().unwrap();
        assert_eq!(pick_count, 5);

        let hit_count: u8 = res
            .attributes.iter().find(|a| a.key == "hit_count").unwrap()
            .value.parse().unwrap();
        assert!(hit_count <= 5);

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");

        if result.value == "win" {
            assert!(res.messages.len() == 1);
            let mult: u128 = res.attributes.iter()
                .find(|a| a.key == "payout_multiplier").unwrap()
                .value.parse().unwrap();
            assert!(mult > 0);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * mult, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(result.value, "lose");
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_keno_single_pick() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "keno_single_player";
        let user_info = mock_info(user, &coins(500_000, "uatom"));

        // 只选 1 个号码
        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayKeno {
                picks: vec![42],
            },
        )
        .unwrap();

        let hit_count: u8 = res
            .attributes.iter().find(|a| a.key == "hit_count").unwrap()
            .value.parse().unwrap();
        let result = res.attributes.iter().find(|a| a.key == "result").unwrap();

        if hit_count == 1 {
            assert_eq!(result.value, "win");
            // 选1中1 赔率 3×
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(1_500_000, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(result.value, "lose");
        }
    }

    #[test]
    fn test_play_keno_max_picks() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(100_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "keno_max_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        // 选 10 个号码
        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayKeno {
                picks: vec![1, 10, 20, 30, 40, 50, 60, 70, 75, 80],
            },
        )
        .unwrap();

        let pick_count: u8 = res
            .attributes.iter().find(|a| a.key == "pick_count").unwrap()
            .value.parse().unwrap();
        assert_eq!(pick_count, 10);

        let result = res.attributes.iter().find(|a| a.key == "result").unwrap();
        assert!(result.value == "win" || result.value == "lose");
    }

    #[test]
    fn test_keno_validation() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        // 空选号
        let err1 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![] },
        );
        assert!(err1.is_err(), "should reject empty picks");

        // 超过 10 个号码
        let err2 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlayKeno {
                picks: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            },
        );
        assert!(err2.is_err(), "should reject more than 10 picks");

        // 号码超出范围
        let err3 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![0, 42] },
        );
        assert!(err3.is_err(), "should reject number 0");

        let err4 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![81] },
        );
        assert!(err4.is_err(), "should reject number > 80");

        // 重复号码
        let err5 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(1_000_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![5, 5, 10] },
        );
        assert!(err5.is_err(), "should reject duplicate numbers");

        // 下注金额过小
        let err6 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(50_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![1] },
        );
        assert!(err6.is_err(), "should reject bet below 100,000");

        // 下注金额过大
        let err7 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(20_000_000, "uatom")),
            ExecuteMsg::PlayKeno { picks: vec![1] },
        );
        assert!(err7.is_err(), "should reject bet above 10,000,000");
    }

    #[test]
    fn test_keno_payout_table() {
        use crate::keno::keno_payout_multiplier;

        // 选1
        assert_eq!(keno_payout_multiplier(1, 0), 0);
        assert_eq!(keno_payout_multiplier(1, 1), 3);

        // 选3
        assert_eq!(keno_payout_multiplier(3, 0), 0);
        assert_eq!(keno_payout_multiplier(3, 1), 0);
        assert_eq!(keno_payout_multiplier(3, 2), 2);
        assert_eq!(keno_payout_multiplier(3, 3), 16);

        // 选5
        assert_eq!(keno_payout_multiplier(5, 2), 0);
        assert_eq!(keno_payout_multiplier(5, 3), 2);
        assert_eq!(keno_payout_multiplier(5, 4), 12);
        assert_eq!(keno_payout_multiplier(5, 5), 50);

        // 选10
        assert_eq!(keno_payout_multiplier(10, 4), 0);
        assert_eq!(keno_payout_multiplier(10, 5), 2);
        assert_eq!(keno_payout_multiplier(10, 10), 2000);
    }

    #[test]
    fn test_keno_draw_no_duplicates() {
        // 验证开出的 20 个号码无重复且在 1-80 范围内
        let info = mock_info("test_user", &coins(1_000_000, "uatom"));
        let env = mock_env();
        let drawn = draw_keno_numbers(&info, &env);

        assert_eq!(drawn.len(), 20, "should draw exactly 20 numbers");

        for &n in &drawn {
            assert!(n >= 1 && n <= 80, "number {} out of range", n);
        }

        // 检查无重复
        let mut unique = drawn.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 20, "drawn numbers should be unique");

        // 验证已排序
        let mut sorted = drawn.clone();
        sorted.sort_unstable();
        assert_eq!(drawn, sorted, "drawn numbers should be sorted");
    }

    // ──────────────────────────────────────────────────────────────────────
    // 刮刮乐（Scratch Card）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_scratch_card_classic() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "scratch_player";
        let bet = 500_000u128;
        let user_info = mock_info(user, &coins(bet, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Classic,
            },
        )
        .unwrap();

        // 检查基本属性
        assert_eq!(
            res.attributes.iter().find(|a| a.key == "action").unwrap().value,
            "play_scratch_card"
        );
        assert_eq!(
            res.attributes.iter().find(|a| a.key == "card_type").unwrap().value,
            "Classic"
        );

        // 检查 3×3 = 9 个格子都有输出
        for row in 1..=3usize {
            for col in 1..=3usize {
                let key = format!("cell_{}_{}", row, col);
                assert!(
                    res.attributes.iter().any(|a| a.key == key),
                    "missing {}",
                    key
                );
            }
        }

        // 检查行 emoji 展示
        assert!(res.attributes.iter().any(|a| a.key == "row_1"));
        assert!(res.attributes.iter().any(|a| a.key == "row_2"));
        assert!(res.attributes.iter().any(|a| a.key == "row_3"));

        // 检查中奖线数
        assert!(res.attributes.iter().any(|a| a.key == "winning_lines"));

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");

        if result.value == "win" {
            assert!(res.messages.len() == 1);
            let mult: u128 = res.attributes.iter()
                .find(|a| a.key == "total_multiplier").unwrap()
                .value.parse().unwrap();
            assert!(mult >= 2, "multiplier should be at least 2");
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * mult, "uatom"),
                }
                .into()
            );
            // 检查 win_desc 存在
            assert!(res.attributes.iter().any(|a| a.key == "win_desc"));
        } else {
            assert_eq!(result.value, "lose");
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_play_scratch_card_premium() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "scratch_premium_player";
        let user_info = mock_info(user, &coins(1_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Premium,
            },
        )
        .unwrap();

        assert_eq!(
            res.attributes.iter().find(|a| a.key == "card_type").unwrap().value,
            "Premium"
        );
        let result = res.attributes.iter().find(|a| a.key == "result").unwrap();
        assert!(result.value == "win" || result.value == "lose");
    }

    #[test]
    fn test_play_scratch_card_deluxe() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "scratch_deluxe_player";
        let user_info = mock_info(user, &coins(2_000_000, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Deluxe,
            },
        )
        .unwrap();

        assert_eq!(
            res.attributes.iter().find(|a| a.key == "card_type").unwrap().value,
            "Deluxe"
        );
    }

    #[test]
    fn test_scratch_card_bet_limits() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        // Classic 下注过小
        let err1 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(50_000, "uatom")),
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Classic,
            },
        );
        assert!(err1.is_err(), "Classic should reject bet below 100,000");

        // Classic 下注过大
        let err2 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(3_000_000, "uatom")),
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Classic,
            },
        );
        assert!(err2.is_err(), "Classic should reject bet above 2,000,000");

        // Premium 下注过小
        let err3 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(200_000, "uatom")),
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Premium,
            },
        );
        assert!(err3.is_err(), "Premium should reject bet below 500,000");

        // Deluxe 下注过小
        let err4 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(500_000, "uatom")),
            ExecuteMsg::PlayScratchCard {
                card_type: ScratchCardType::Deluxe,
            },
        );
        assert!(err4.is_err(), "Deluxe should reject bet below 1,000,000");
    }

    #[test]
    fn test_scratch_card_evaluation_logic() {
        use crate::scratch::{evaluate_scratch_card, ScratchSymbol};

        // 全部相同 → 8 条线全中
        let all_diamond = [ScratchSymbol::Diamond; 9];
        let (mult, lines) = evaluate_scratch_card(&all_diamond);
        assert_eq!(lines.len(), 8, "all same should win 8 lines");
        assert_eq!(mult, 50 * 8, "8 lines × 50× each");

        // 第一行全 Cherry → 1 条线
        let mut grid = [ScratchSymbol::Lemon; 9];
        grid[0] = ScratchSymbol::Cherry;
        grid[1] = ScratchSymbol::Cherry;
        grid[2] = ScratchSymbol::Cherry;
        let (mult2, lines2) = evaluate_scratch_card(&grid);
        // row1 中奖 Cherry(3×)，row2 和 row3 全 Lemon(2×)，列和对角线不一定
        // row2: [Lemon, Lemon, Lemon] → 中，row3: [Lemon, Lemon, Lemon] → 中
        // col1: [Cherry, Lemon, Lemon] → 不中
        // 需要精确计算
        assert!(lines2.iter().any(|l| l.0 == "row1"), "row1 should win");
        assert!(lines2.iter().any(|l| l.0 == "row2"), "row2 (all Lemon) should win");
        assert!(lines2.iter().any(|l| l.0 == "row3"), "row3 (all Lemon) should win");
        assert_eq!(mult2, 3 + 2 + 2, "Cherry 3× + Lemon 2× + Lemon 2× = 7×");

        // 无任何三连 → 0
        let no_match = [
            ScratchSymbol::Diamond, ScratchSymbol::Star,    ScratchSymbol::Clover,
            ScratchSymbol::Bell,    ScratchSymbol::Cherry,  ScratchSymbol::Lemon,
            ScratchSymbol::Star,    ScratchSymbol::Diamond, ScratchSymbol::Bell,
        ];
        let (mult3, lines3) = evaluate_scratch_card(&no_match);
        assert_eq!(mult3, 0);
        assert_eq!(lines3.len(), 0);

        // 对角线中奖
        let mut diag_grid = [ScratchSymbol::Lemon; 9];
        diag_grid[0] = ScratchSymbol::Star;
        diag_grid[4] = ScratchSymbol::Star;
        diag_grid[8] = ScratchSymbol::Star;
        // grid: Star Lemon Lemon / Lemon Star Lemon / Lemon Lemon Star
        // diag1: [0,4,8] = Star → 中
        // row2: [Lemon, Star, Lemon] → 不中
        // 但 row1 和 row3 没有全 Lemon（因为有 Star 在角上）
        // row1: [Star, Lemon, Lemon] → 不中
        let (mult4, lines4) = evaluate_scratch_card(&diag_grid);
        assert!(lines4.iter().any(|l| l.0 == "diag1"), "diag1 should win");
        // diag1 中 Star(20×) 只有这一条
        assert!(mult4 >= 20, "at least Star 20× for diag1");
    }

    // ──────────────────────────────────────────────────────────────────────
    // 斗牛（Bull Fight / Niu Niu）测试
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_play_bullfight() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        let user = "bull_player";
        let bet = 1_000_000u128;
        let user_info = mock_info(user, &coins(bet, "uatom"));

        let res = execute(
            deps.as_mut(),
            mock_env(),
            user_info,
            ExecuteMsg::PlayBullFight {},
        )
        .unwrap();

        // 检查基本属性
        assert_eq!(
            res.attributes.iter().find(|a| a.key == "action").unwrap().value,
            "play_bullfight"
        );
        assert!(res.attributes.iter().any(|a| a.key == "player_cards"));
        assert!(res.attributes.iter().any(|a| a.key == "dealer_cards"));
        assert!(res.attributes.iter().any(|a| a.key == "player_hand_type"));
        assert!(res.attributes.iter().any(|a| a.key == "dealer_hand_type"));
        assert!(res.attributes.iter().any(|a| a.key == "player_score"));
        assert!(res.attributes.iter().any(|a| a.key == "dealer_score"));

        let result = res.attributes.iter().find(|a| a.key == "result").expect("result missing");

        if result.value == "win" {
            assert_eq!(res.messages.len(), 1);
            let mult: u128 = res.attributes.iter()
                .find(|a| a.key == "payout_multiplier").unwrap()
                .value.parse().unwrap();
            assert!(mult >= 2 && mult <= 8, "multiplier should be 2-8, got {}", mult);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet * mult, "uatom"),
                }
                .into()
            );
        } else if result.value == "tie" {
            assert_eq!(res.messages.len(), 1);
            assert_eq!(
                res.messages[0].msg,
                BankMsg::Send {
                    to_address: user.to_string(),
                    amount: coins(bet, "uatom"),
                }
                .into()
            );
        } else {
            assert_eq!(result.value, "lose");
            assert_eq!(res.messages.len(), 0);
        }
    }

    #[test]
    fn test_bullfight_bet_limits() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {};
        let creator_info = mock_info("creator", &coins(10_000_000_000, "uatom"));
        instantiate(deps.as_mut(), mock_env(), creator_info, msg).unwrap();

        // 下注过小
        let err1 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(50_000, "uatom")),
            ExecuteMsg::PlayBullFight {},
        );
        assert!(err1.is_err(), "should reject bet below 100,000");

        // 下注过大
        let err2 = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &coins(20_000_000, "uatom")),
            ExecuteMsg::PlayBullFight {},
        );
        assert!(err2.is_err(), "should reject bet above 10,000,000");
    }

    #[test]
    fn test_bullfight_hand_evaluation() {
        use crate::bullfight::{evaluate_bull_hand, BullCard, BullHandType, Suit};

        // ── 五小牛：A A A 2 3 → 全 ≤5，总和=1+1+1+2+3=8 ≤10 ──
        let wu_xiao = [
            BullCard { rank: 1, suit: Suit::Spades },
            BullCard { rank: 1, suit: Suit::Hearts },
            BullCard { rank: 1, suit: Suit::Diamonds },
            BullCard { rank: 2, suit: Suit::Clubs },
            BullCard { rank: 3, suit: Suit::Spades },
        ];
        let r1 = evaluate_bull_hand(&wu_xiao);
        assert_eq!(r1.hand_type, BullHandType::WuXiaoNiu);
        assert!(r1.score >= 90_000);

        // ── 四炸：K K K K 5 ──
        let si_zha = [
            BullCard { rank: 13, suit: Suit::Spades },
            BullCard { rank: 13, suit: Suit::Hearts },
            BullCard { rank: 13, suit: Suit::Diamonds },
            BullCard { rank: 13, suit: Suit::Clubs },
            BullCard { rank: 5, suit: Suit::Spades },
        ];
        let r2 = evaluate_bull_hand(&si_zha);
        assert!(matches!(r2.hand_type, BullHandType::SiZha { rank: 13 }));
        assert!(r2.score >= 80_000);

        // ── 五花牛：J Q K J Q ──
        let wu_hua = [
            BullCard { rank: 11, suit: Suit::Spades },
            BullCard { rank: 12, suit: Suit::Hearts },
            BullCard { rank: 13, suit: Suit::Diamonds },
            BullCard { rank: 11, suit: Suit::Clubs },
            BullCard { rank: 12, suit: Suit::Diamonds },
        ];
        let r3 = evaluate_bull_hand(&wu_hua);
        assert_eq!(r3.hand_type, BullHandType::WuHuaNiu);
        assert!(r3.score >= 70_000);

        // ── 牛牛：10 10 10 J K → 三张 10+10+10=30(10倍), 剩余 10+10=20(10倍) ──
        let niu_niu = [
            BullCard { rank: 10, suit: Suit::Spades },
            BullCard { rank: 10, suit: Suit::Hearts },
            BullCard { rank: 10, suit: Suit::Diamonds },
            BullCard { rank: 11, suit: Suit::Clubs },
            BullCard { rank: 13, suit: Suit::Spades },
        ];
        let r4 = evaluate_bull_hand(&niu_niu);
        assert_eq!(r4.hand_type, BullHandType::NiuNiu);
        assert!(r4.score >= 60_000);

        // ── 牛 7：10 J 10 3 4 → 10+10+10=30(牛), 3+4=7 ──
        let niu7 = [
            BullCard { rank: 10, suit: Suit::Spades },
            BullCard { rank: 11, suit: Suit::Hearts },
            BullCard { rank: 10, suit: Suit::Diamonds },
            BullCard { rank: 3, suit: Suit::Clubs },
            BullCard { rank: 4, suit: Suit::Spades },
        ];
        let r5 = evaluate_bull_hand(&niu7);
        assert_eq!(r5.hand_type, BullHandType::NiuN { n: 7 });
        assert!(r5.score >= 50_000);

        // ── 没牛：A 2 6 8 K → 无法选 3 张凑成 10 的倍数 ──
        // 检查：1+2+6=9, 1+2+10=13, 1+6+8=15, 1+6+10=17, 1+8+10=19
        //       2+6+8=16, 2+6+10=18, 2+8+10=20✓ → 有牛，剩余 1+6=7 → 牛7
        // 用 A 3 6 8 K: 1+3+6=10✓ → 牛8+10=18%10=8 → 牛8
        // 用另一组: A 2 3 6 8: 1+2+3=6, 1+2+6=9, 1+2+8=11, 1+3+6=10✓ → 牛 2+8=10%10=0 → 牛牛
        // 真正没牛的例子：A 2 3 7 K: 1+2+3=6, 1+2+7=10✓ → 有牛，3+10=13%10=3 → 牛3
        // 更好的没牛：A 2 4 7 K: 1+2+4=7, 1+2+7=10✓ → 牛 4+10=14%10=4
        // 直接构造：3 4 6 7 8 → 3+4+6=13, 3+4+7=14, 3+4+8=15, 3+6+7=16, 3+6+8=17,
        //                       3+7+8=18, 4+6+7=17, 4+6+8=18, 4+7+8=19, 6+7+8=21
        // 没有 10 的倍数 → 没牛！
        let no_niu = [
            BullCard { rank: 3, suit: Suit::Spades },
            BullCard { rank: 4, suit: Suit::Hearts },
            BullCard { rank: 6, suit: Suit::Diamonds },
            BullCard { rank: 7, suit: Suit::Clubs },
            BullCard { rank: 8, suit: Suit::Spades },
        ];
        let r6 = evaluate_bull_hand(&no_niu);
        assert_eq!(r6.hand_type, BullHandType::NoNiu);
        assert!(r6.score < 50_000);

        // ── 牌型强弱排序 ──
        assert!(r1.score > r2.score, "五小牛 > 四炸");
        assert!(r2.score > r3.score, "四炸 > 五花牛");
        assert!(r3.score > r4.score, "五花牛 > 牛牛");
        assert!(r4.score > r5.score, "牛牛 > 牛7");
        assert!(r5.score > r6.score, "牛7 > 没牛");
    }

    #[test]
    fn test_bullfight_payout_multipliers() {
        use crate::bullfight::{bull_payout_multiplier, BullHandType};

        assert_eq!(bull_payout_multiplier(&BullHandType::WuXiaoNiu), 8);
        assert_eq!(bull_payout_multiplier(&BullHandType::SiZha { rank: 10 }), 7);
        assert_eq!(bull_payout_multiplier(&BullHandType::WuHuaNiu), 6);
        assert_eq!(bull_payout_multiplier(&BullHandType::NiuNiu), 4);
        assert_eq!(bull_payout_multiplier(&BullHandType::NiuN { n: 9 }), 3);
        assert_eq!(bull_payout_multiplier(&BullHandType::NiuN { n: 8 }), 3);
        assert_eq!(bull_payout_multiplier(&BullHandType::NiuN { n: 7 }), 2);
        assert_eq!(bull_payout_multiplier(&BullHandType::NiuN { n: 1 }), 2);
        assert_eq!(bull_payout_multiplier(&BullHandType::NoNiu), 2);
    }
}
