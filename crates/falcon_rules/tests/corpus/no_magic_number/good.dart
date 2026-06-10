// Good: using named constants instead of magic numbers
const kPadding = 16.0;
const kThreshold = 0.75;
const kMaxRetries = 10;
const kTimeoutMs = 1000;

class Theme {
  final padding = kPadding;
  final threshold = kThreshold;
}

void configureUI() {
  // Good: allowed magic numbers (0, 1, 2, -1)
  int counter = 0;
  int flag = 1;
  int pair = 2;
  int delta = -1;

  // Good: using named constants
  int maxRetries = kMaxRetries;
  int timeout = kTimeoutMs;
}

// Good: list literals with allowed numbers
void getValues() {
  final list = [0, 1, 2, -1];
}
