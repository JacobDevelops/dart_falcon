---
default: minor
---

Avoid directly nesting SDK streams and futures: `Stream<Future<T>>` and `Future<Stream<T>>` complicate consumption.
