---
default: patch
---

Limit `prefer-correct-test-file-name` to package-root `test/` and `integration_test/` directories so Flutter driver entrypoints and helpers under `test_driver/` or `testing/` don't require the `_test.dart` suffix.
