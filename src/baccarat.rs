use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BaccaratBet {
    Player,
    Banker,
    Tie,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BaccaratResult {
    pub player_cards: Vec<u8>,
    pub banker_cards: Vec<u8>,
    pub player_total: u8,
    pub banker_total: u8,
    pub winner: BaccaratBet,
}