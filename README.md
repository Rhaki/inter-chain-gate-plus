# Interchain Gate Plus

This repository contains examples of contracts that integrate [gate-contract](https://github.com/BIG-Labs/gate-contract-core) and some ibc-enable contracts.

## Gate contracts

Name | Description
|-|-|
| [market](/contracts/market/) | Basic `money market` contract that allow user to deposit `token` (native or `cw20-icg`), take a fake loan and bridge their positions using `gate` |
| [oracle](/contracts/oracle/) | Oracle feed contract that can use `gate` contract to query prices to another chain, and then save the price response on its state |


## Other contracts

Name | Description
|-|-|
| [ibc-hooker](/contracts/ibc-hooker/) | Allow to send funds via ics-20 `MsgTransfer` to a remote version of `ibc-hooker` contract, allowing to specify a path for [packet-forward-middleware](https://github.com/strangelove-ventures/packet-forward-middleware), and if the tx on remote chain has to fail. It's a perfect example to test `ibc-hook` + `packet-forward-middleware` + `ibc_callback`



