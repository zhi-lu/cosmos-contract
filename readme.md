# Cosmos 链游智能合约 / Cosmos Chain Game Smart Contract

## 合约编译方式(Docker) / Contract Compilation (Docker)

```shell
docker run --rm -v "$(pwd)":/code \                                                  
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

## 合约部署方式(Wasmd) / Contract Deployment (Wasmd)

```shell
# 合约存储到链上 / Store contract on chain
wasmd tx wasm store ./artifacts/play_contract.wasm  --from "wasmxxxxxxxxxx" --gas-prices 0.001uatom --gas 2000000  --broadcast-mode sync

# 创建合约实例 / Instantiate contract
wasmd tx wasm instantiate your_contract_id '{}' \                                                                                     
  --from wasmxxxxxxxxxx \
  --label "play_game" \
  --admin wasmxxxxxxxxxx \
  --amount "10000000000uatom" \
  --gas-prices 0.001uatom \
  --gas 200000 \
  --broadcast-mode sync \
  -y
  
# 查询交易 / Query transaction
curl https://${IP}:${REST_PORT}/cosmos/tx/v1beta1/txs/${tx_hash}
```

## 目前支持的游戏 / Currently Supported Games

| 中文名称   | English Name      | Description                                    |
|--------|-------------------|------------------------------------------------|
| 大小游戏   | Double or Nothing | 经典双倍或清零游戏 / Classic Double or Zero Game        |
| 老虎机游戏  | Slots             | 传统老虎机玩法   /  Traditional slot machine gameplay |
| 猜数字游戏  | Guess Number      | 数字猜测游戏    / Number Guessing Game               |
| 21 点游戏 | Mini Blackjack    | 简化版21点玩法   / Simplified Blackjack gameplay     |
| 硬币抛掷   | Coin Flip         | 正反面猜测游戏   / Heads and tails guessing game      |
| 骰子对赌   | Dice Roll Duel    | 骰子对战游戏     / Dice Battle Game                  |
| 幸运转盘   | Lucky Wheel       | 轮盘抽奖游戏     / Roulette Game                     |
| 百家乐    | Baccarat          | 简单的百家乐游戏    / Simplified Baccarat Game         |

## 使用说明 / Usage Notes

### 环境要求 / Requirements

1. Docker v20.10+
2. wasmd v0.45.0+

### 注意事项 / Considerations

1. 部署前请确保有足够的代币余额 / Please ensure that you have sufficient token balance before deployment
2. 建议在测试网先进行测试 / It is recommended to test on testnet first