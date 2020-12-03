# Mirror Mint <!-- omit in toc -->

**NOTE**: Reference documentation for this contract is available [here](https://docs.mirror.finance/contracts/mint).

The Mint Contract implements the logic for Collateralized Debt Positions (CDPs), through which users can mint new mAsset tokens against their deposited collateral (UST or mAssets). Current prices of collateral and minted mAssets are read from the Oracle Contract determine the C-ratio of each CDP. The Mint Contract also contains the logic for liquidating CDPs with C-ratios below the minimum for their minted mAsset through auction.
