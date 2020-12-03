# Mirror Staking <!-- omit in toc -->

**NOTE**: Reference documentation for this contract is available [here](https://docs.mirror.finance/contracts/staking).

The Staking Contract contains the logic for LP Token staking and reward distribution. Staking rewards for LP stakers come from the new MIR tokens generated at each block by the Factory Contract and are split between all combined staking pools. The new MIR tokens are distributed in proportion to size of staked LP tokens multiplied by the weight of that asset's staking pool.
