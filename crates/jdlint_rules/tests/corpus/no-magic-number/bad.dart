// Test cases for no-magic-number rule
// Flags numeric literals not in allowlist [0, 1, 2, -1]
// Phase 1: Simple allowlist check. Config: allowlist in jdlint.json.

void testMagicTimeout() {
  final timeout = 30; /* expect: no-magic-number */
  sleep(timeout);
}

void testMagicThreshold() {
  if (count > 100) { /* expect: no-magic-number */
    print("over threshold");
  }
}

void testMagicPadding() {
  final padding = EdgeInsets.all(16); /* expect: no-magic-number */
}

void testMultipleMagicNumbers() {
  final width = 800; /* expect: no-magic-number */
  final height = 600; /* expect: no-magic-number */
  final maxRetries = 5; /* expect: no-magic-number */
}

class Config {
  static final defaultPort = 8080; /* expect: no-magic-number */
  static final timeout = 3000; /* expect: no-magic-number */
  static final maxConnections = 50; /* expect: no-magic-number */
}

void testMagicInOperations() {
  final result = value * 100; /* expect: no-magic-number */
  final scaled = base + 20; /* expect: no-magic-number */
  final divided = total / 3; /* expect: no-magic-number */
}

void testMagicInLoop() {
  for (int i = 0; i < 10; i++) { /* expect: no-magic-number */
    print(i);
  }
}

void testMagicInRange() {
  final list = List.generate(25, (i) => i); /* expect: no-magic-number */
}

void testAllowedNumbers() {
  final zero = 0;
  final one = 1;
  final two = 2;
  final negOne = -1;
}
