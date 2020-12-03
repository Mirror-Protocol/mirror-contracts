# Mirror Factory

**NOTE**: Reference documentation for this contract is available [here](https://docs.mirror.finance/contracts/factory).

The Factory contract is Mirror Protocol's central directory and organizes information related to mAssets and the Mirror Token (MIR). It is also responsible for minting new MIR tokens each block and distributing them to the Staking Contract for rewarding LP Token stakers.

After the initial bootstrapping of Mirror Protocol contracts, the Factory is assigned to be the owner for the Mint, Oracle, Staking, and Collector contracts. The Factory is owned by the Gov Contract.
