---
default: minor
---
Implement `require-atomic-async-updates` to detect assignments that reuse a value read before an asynchronous suspension, excluding uncaptured local variables and parameters.
