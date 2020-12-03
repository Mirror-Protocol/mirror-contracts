# Mirror Oracle <!-- omit in toc -->

**NOTE**: Reference documentation for this contract is available [here](https://docs.mirror.finance/contracts/oracle).

The Oracle Contract exposes an interface for accessing the latest reported price for mAssets. Price quotes are kept up-to-date by oracle feeders that are tasked with periodically fetching exchange rates from reputable sources and reporting them to the Oracle contract.

Prices are only considered valid for 60 seconds. If no new prices are published after the data has expired, Mirror will disable CDP operations (mint, burn, deposit, withdraw) until the price feed resumes.
