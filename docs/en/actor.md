# Actor experiments

An actor receives messages and updates its own state. Nagi has an experimental counter actor. There is no syntax for defining arbitrary user actors yet.

[actor.nagi](../../examples/actor.nagi) sends messages between a forwarding actor and a counter actor. Only the counter actor changes its state. Its mailbox holds 64 messages; senders wait when it is full.

The test compares waiting for each reply with sending 32 messages before reading their replies. Both receive replies and wait for the actors to finish at shutdown.

This is not a general-purpose actor API. Persistent messages, redelivery after failure, and distribution across machines are not supported.
