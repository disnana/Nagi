# Actors

The runtime includes a counter actor with a bounded mailbox and oneshot replies. Only its actor task updates the counter's mutable state. High does not yet have an `actor` declaration or support generating arbitrary actor state and methods.

`examples/actor.nagi` runs a forwarder actor → counter actor → forwarder → caller exchange. The actors communicate through channels without sharing counter state. After all senders are dropped, their tasks are joined.

Awaiting one reply at a time makes frequent trips through the scheduler. The runtime's pipelined version sends 32 messages before checking their replies. This comparison retains replies; it does not compare against fire-and-forget delivery.

The mailbox holds 64 messages. Senders await space when it is full. Message redelivery after failure, persistent mailboxes, and actor placement across machines are not implemented.
