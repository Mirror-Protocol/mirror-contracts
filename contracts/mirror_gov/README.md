# Voting

This is a simple voting contract. It creates a contract to manage token weighted polls,
where voters deposit predefined gov cw20 tokens in order to vote.
Voters can withdraw their stake, but not while a poll they've participated in is still in progress.

Anyone can create a poll, and as the poll creator, only they are allowed to end/tally the poll.

## End Poll

If the polls is ended with `pass`, it will return `WasmMsg::Execute` msg with same data which is set at poll creation step.
