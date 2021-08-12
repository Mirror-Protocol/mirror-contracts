# Mirror Collateral Oracle <!-- omit in toc -->

**NOTE**: Reference documentation for this contract is available [here](https://docs.mirror.finance/contracts/collateral-oracle).

Collateral Oracle contract manages a directory of whitelisted collateral assets, providing the necessary interfaces to register and revoke assets. Mint contract will fetch prices from collateral oracle to determine the C-ratio of each CDP. 

The Collateral Oracle fetches prices from different sources on the Terra ecosystem, acting as a proxy for Mint Contract.