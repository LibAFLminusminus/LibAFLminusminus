# Baby corpus sharing

This example shows a simple example demonstrating corpus sharing between two workers:
- `worker1`, which is your average average in-process client
- `worker2`, which is a nop fuzzer (no mutation is performed). It will only evaluate inputs received from `worker1`.

We will create an unidirectional link, going from `worker1` to `worker2`.
The test succeed if `worker2` has an evolving corpus, which demonstrates it is able to receive inputs form `worker1`.
