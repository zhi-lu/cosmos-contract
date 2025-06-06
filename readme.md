# Cosmos 链游智能合约

## 合约编译方式(Docker)

```shell
docker run --rm -v "$(pwd)":/code \                                                  
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

## 合约部署方式(Wasmd)

```shell
# 合约存储到链上
wasmd tx wasm store ./artifacts/play_contract.wasm  --from "wasmxxxxxxxxxx" --gas-prices 0.001uatom --gas 2000000  --broadcast-mode sync

# 创建合约实例
wasmd tx wasm instantiate your_contract_id '{}' \                                                                                     
  --from wasmxxxxxxxxxx \
  --label "play_game" \
  --admin wasmxxxxxxxxxx \
  --amount "10000000000uatom" \
  --gas-prices 0.001uatom \
  --gas 200000 \
  --broadcast-mode sync \
  --node http://127.0.0.1:11007 \
  -y
  
# 查询交易
curl https://${IP}:${REST_PORT}/cosmos/tx/v1beta1/txs/${tx_hash}
```

## 目前支持的游戏

1. 比大小游戏(double or nothing )
2. 老虎机游戏(slots)
3. 猜数字游戏(guess number)
4. 21 点游戏简易版(Mini Blackjack)
5. 硬币抛掷(Coin Flip)
6. 骰子对赌(Dice Roll Duel)
7. 幸运转盘(Lucky Wheel)