use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{log, near_bindgen};


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
}

impl Default for Contract{
    fn default() -> Self{
        Self{}
    }
}

#[near_bindgen]
impl Contract {
}
