# Oracle Contract

This contract is a OCS conformant smart contract. It provides simple interfcae to feed the price from the authorized feeder key. It also provides the variable `price_multiplier` to cope with events like stock split or merge. The oracle users can calculate the active price by multipling `price` and `price_multiplier`.

## Handlers

### Feed Price

Only authorized feeder addresses are allowed to report prices. It provides an interface to update multiple prices at once from a feeder address.

Request Format
* Feed Price

   ```json
   { 
       "feed_price": {
           "price_infos": [
                { 
                    "asset_info": {
                       "token": { "contract_addr": "terra~~~" }
                    }, 
                    "price": "1300.0" 
                },
                { 
                    "asset_info": {
                       "token": { "contract_addr": "terra~~~" }
                    }, 
                    "price": "1.3" 
                },
                ...
            ]
        },
    }
   ```

* Feed Price with Price Multiplier
    
   A feeder also can do update price multiplier, when there is some event on asset. The updated price multiplier replaces origin one so the feeder do not need to feed it more than once.

   ```json
      { 
       "feed_price": {
           "price_infos": [
                { 
                    "asset_info": {
                       "token": { "contract_addr": "terra~~~" }
                    }, 
                    "price": "1300.0",
                    "price_multiplier": "1.2",
                },
                { 
                    "asset_info": {
                       "token": { "contract_addr": "terra~~~" }
                    }, 
                    "price": "1.3" 
                },
                ...
            ]
        },
    }
   ```

### Update Config

The owner also can do update and `owner` by sending `update_config` msg.

```json
{ "update_config": { "owner": "terra~~" } }
```

### RegisterAsset

The owner can register new asset and also can update the feeder for a specific asset.

Request Format

```json
{ "register_assset": {
    "asset_info": { 
        "token": {
            "contract_addr": "terra~~"
        },
        "feeder": "terra~~",
    }
}
```

## Queriers
### Config
Query interface for config data.

Request

```json
{"config": "{}}
```

Response

```json
{
    "owner": "terra~~",
    "base_asset_info": {
        "native_token": {
            "denom": "uusd"
        }
    }
}
```

### Asset
Query interface for asset info

Request

```json
{
    "asset": {
        "asset_info": {
            "token": {
                "contract_addr": "terra~~",
            }
        }
    }
}
```

Response

```json
{
    "asset_info": {
        "token": {
            "contract_addr": "terra~~",
        }
    },
    "feeder": "terra~~"
}
```

### Price
Query interface for oracle price of an asset

Request

```json
{
    "price": {
        "asset_info": {
            "token": {
                "contract_addr": "terra~~",
            }
        }
    }
}
```

Response

```json
{
    "price": "1300.0",
    "price_multiplier": "1.2",
    "last_update_time": 1023832823,
    "asset_info": {
        "token": {
            "contract_addr": "terra~~",
        }
    }
}
```

### Prices
Query interface for all price

```json
{ "prices": {} }
```

Response

```json
{
    "prices": [
        {
            "price": "1300.0",
            "price_multiplier": "1.2",
            "last_update_time": 1023832823,
            "asset_info": {
                "token": {
                    "contract_addr": "terra~~",
                }
            }
        }
        ...
    ]    
}
```
