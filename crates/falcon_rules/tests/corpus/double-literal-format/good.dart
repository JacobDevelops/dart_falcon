// Good cases for double-literal-format rule
// No violations expected

void testCorrectLeadingZero() {
  final opacity = 0.5;
  final scale = 0.75;
  final threshold = 0.001;
  final value = 0.999;
}

void testIntegersAsIntegers() {
  const duration = 1;
  const timeout = 2;
  final ratio = 10;
  final factor = 5;
}

// dcl checks literal *formatting* only. A trailing zero whose fractional part
// is exactly "0" (`1.0`, `24.0`) is NOT flagged: stripping it would turn a
// `double` into an `int`, which is a different value, not a reformat.
void testTrailingSingleZeroIsFine() {
  const duration = 1.0;
  const timeout = 2.0;
  final ratio = 10.0;
  final factor = 5.0;
  final zero = 0.0;
  const big = 100.0;
}

class Animation {
  final double speed = 0.5;
  final double delay = 1.5;
  final double curve = 0.25;
  final double full = 1.0;
}

double calculateOpacity() {
  return 0.8;
}

void processValues() {
  final list = [0.1, 0.2, 0.3];
  final map = {
    'x': 0.5,
    'y': 1.5,
  };
}

void mathOperations() {
  final result = 0.5 + 0.25;
  final product = 2.5 * 0.75;
}

bool validateRange(double value) {
  return value > 0.0;
}

void mixedFormats() {
  final a = 0.5;
  final b = 3.5;
  final c = 0.125;
  final d = 100.5;
}

void properFormatting() {
  const pi = 3.14159;
  final sqrtTwo = 1.414;
  final goldenRatio = 1.618;
}

class Percentage {
  final double value = 0.5;

  bool isValid() => value >= 0.0 && value <= 1;
}

void performanceMetrics() {
  final cpuUsage = 0.85;
  final memoryUsage = 0.92;
  final diskUsage = 0.78;
}
