# Supervised reservation workers

Separate actors manage morning and evening reservations. Each starts with ten seats. Duplicate booking references and requests exceeding availability return a custom `BookingError`, preserving the current state.

```sh
nagic run --project test-nagi-code/application-examples/seat-reservations/nagi.toml
```

Successful application output:

```text
reservations: validated duplicates, capacity, restart and sibling isolation
```

The application checks successful bookings, duplicates, and sold-out replies, then deliberately fails the morning worker. A bounded polling loop verifies its supervised restart. The evening worker keeps its existing reservation.

Bookings are stored only in memory. Restarting a worker loses its reservations and restores all ten seats. This example does not implement persistence or payments.

`smoke.py` limits execution time and checks the exit code and output. Nagi assertions distinguish business failures from worker failures. See the [parent README](../README.en.md) for the shared verification command.
