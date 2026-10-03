# Worker restart experiments

The runtime tests restarting a worker after it panics while other workers continue processing.

[supervisor.nagi](../../examples/supervisor.nagi) deliberately causes three panics. Panic logs are expected in this test. At normal shutdown, remaining workers are asked to stop and then joined.

A restart waits one millisecond. The number of restarts within one second is limited so a repeatedly failing worker eventually stops restarting.

If the parent operation is dropped, running workers are also asked to stop. Dropping it does not wait for every worker to finish.

An API for defining worker trees and restart policies is not implemented. HTTP requests are not redelivered. Handling a panic does not provide recovery from OS process termination or memory corruption.
