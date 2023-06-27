This contract show how use `ibc hook` with `ibc_callback`.
A `Request` can be send as `ExecuteMsg` with some native coins, speicfing
- the `channel-id` to use (ports on both chain must to be `transfer`);
- the `ibc_hook` address on dest chain;
- the `receiver` address on dest chain;
- if the tx on dest chain have to fail or not (this is used to test the ack);

If the tx fails the ack come inside the contract from `sudo` entrypoint and the contract send back the native token to the original `sender`.

No safety check has been implemented.