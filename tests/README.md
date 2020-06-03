Integration tests
=================

These integration test are meant to be running on the a drone CI server where a precise environment is created.
If you wish to run the tests locally, make sure you have a tezedge-debugger instance set up and running corectly then run `cargo test`.
(It is recommended to wait a little after the start of the debugger to have access to a few messages in the databse)