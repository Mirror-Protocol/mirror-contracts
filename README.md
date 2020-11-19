# Mirror Protocol Contracts

This repository contains the smart contracts that implement Mirror Protocol on [Terra](https://terra.money). For more information about Mirror Protocol, please visit the official documentation site [here](https://docs.mirror.finance).

Mirror depends on [Terraswap](https://terraswap.org) and uses its [implementation](https://github.com/terraswap/terraswap) of the CW20 token specification.

## Contracts

| Name                                                         | Description                      |
| ------------------------------------------------------------ | -------------------------------- |
| [`mirror_collector`](./contracts/mirror_collector/README.md) | reward collector                 |
| [`mirror_factory`](./contracts/mirror_factory/README.md)     | controls whitelisting of mAssets |
| [`mirror_gov`](./contracts/mirror_gov/README.md)             | controls governance              |
| [`mirror_mint`](./contracts/mirror_mint/README.md)           | mAsset minting and burning logic |
| [`mirror_oracle`](./contracts/mirror_oracle/README.md)       | controls the oracle feeder       |
| [`mirror_staking`](./contracts/mirror_staking/README.md)     | controls staking functions       |

## Initialization

**NOTE:** mAPPL will be used as an example.

- Mirror contracts should be instantiated in the following order:

  1. `mirror.factory`
  2. `terraswap.token` (MIR token)
  3. `mirror.gov`
  4. `mirror.oracle`
  5. `mirror.mint`
  6. `mirror.staking`
  7. `terraswap.factory`
  8. `mirror.collector`
  9. `terraswap.token` (mAPPL token)

- The pair for (MIR/UST) is created:

  - `Terraswap_factory.create_pair` (MIR/UST)
  - `mirror.factory.post_initialize`
  - `mirror.factory.terraswap_creation_hook(MIR)`: whitelist MIR
  - `mirror.factory.whitelist(APPL)`: whitelist mAPPL
  - `mirror.factory.update_owner(gov)`: gov contract now owns the factory

## Development

### Environment Setup

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker

1. Install `rustup` via https://rustup.rs/

2. Run the following:

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Make sure [Docker](https://www.docker.com/) is installed

### Unit / Integration Tests

Each contract contains Rust unit and integration tests embedded within the contract source directories. You can run:

```sh
cargo unit-test
cargo integration-test
```

### Compiling

After making sure tests pass, you can compile each contract with the following:

```sh
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw1_subkeys.wasm .
ls -l cw1_subkeys.wasm
sha256sum cw1_subkeys.wasm
```

#### Production

For production builds, run the following:

```sh
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.10.2
```

This performs several optimizations which can significantly reduce the final size of the contract binaries, which will be available inside the `artifacts/` directory.

## License

This software is licensed under the Apache 2.0 license. Read more about it [here](LICENSE.md).
