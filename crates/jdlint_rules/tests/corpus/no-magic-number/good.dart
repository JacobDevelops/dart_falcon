// Good examples for no-magic-number rule
// Using named constants instead of magic numbers

const int kDefaultTimeout = 30;
const int kMaxRetries = 5;
const int kMaxConnections = 50;
const int kDefaultPort = 8080;
const int kMaxDimension = 800;
const int kMinDimension = 600;

void testWithNamedConstant() {
  final timeout = kDefaultTimeout;
  sleep(timeout);
}

void testThresholdWithConstant() {
  const int threshold = 100;
  if (count > threshold) {
    print("over threshold");
  }
}

void testPaddingWithConstant() {
  final padding = EdgeInsets.all(kDefaultPadding);
}

class Config {
  static final int defaultPort = kDefaultPort;
  static final int timeout = 3000;
  static final int maxConnections = kMaxConnections;
}

void testAllowedNumbersUsage() {
  final zero = 0;
  final one = 1;
  final two = 2;
  final negOne = -1;
}

void testIndexAccess() {
  final first = list[0];
  final last = list[list.length - 1];
}

void testLoopWithConstant() {
  const int iterations = 10;
  for (int i = 0; i < iterations; i++) {
    print(i);
  }
}

void testGenerateWithConstant() {
  const int size = 25;
  final list = List.generate(size, (i) => i);
}

void testMathOperations() {
  const int base = 100;
  final result = value * base;
  const int increment = 20;
  final scaled = base + increment;
}

void testOffsets() {
  final position = offset + 0;
  final previous = index - 1;
}
