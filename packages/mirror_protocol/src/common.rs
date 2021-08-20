use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Order;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderBy {
    Asc,
    Desc,
}

// impl Into<Order> for OrderBy {
//     fn into(self) -> Order {
//         if self == OrderBy::Asc {
//             Order::Ascending
//         } else {
//             Order::Descending
//         }
//     }
// }

impl From<OrderBy> for Order {
    fn from(order_by: OrderBy) -> Order {
        if order_by == OrderBy::Asc {
            Order::Ascending
        } else {
            Order::Descending
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Mainnet,
    Testnet,
}
