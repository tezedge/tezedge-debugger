Integration tests
=================

These integration tests are meant to be running on the a drone CI server where a precise environment is created.
If you wish to run the tests locally, make sure you have a tezedge-debugger instance set up and running correctly.
(It is recommended to wait a little after the start of the debugger to have access to a few messages in the database)


### Example run
```
DEBUGGER_URL=http://116.202.128.230:17732 NODE_URL=http://116.202.128.230:18732 WEBSOCKET_PORT=4972 cargo test -- --ignored ocaml
```

Note that the integration test are marked as ignored so they won't run without the `--ignored` flag. After the flag, the node type is given (either `ocaml` or `rust`)