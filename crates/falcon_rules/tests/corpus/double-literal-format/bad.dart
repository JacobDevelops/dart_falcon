// Test cases for double-literal-format rule
// All violations are marked inline below.

void testMissingLeadingZero() {
  final opacity = .5; /* expect: double-literal-format */
  final scale = .75; /* expect: double-literal-format */
  final threshold = .001; /* expect: double-literal-format */
  final value = .999; /* expect: double-literal-format */
}

void testUnnecessaryTrailingZeros() {
  const duration = 1.0; /* expect: double-literal-format */
  const timeout = 2.0; /* expect: double-literal-format */
  final ratio = 10.0; /* expect: double-literal-format */
  final factor = 5.0; /* expect: double-literal-format */
}

class Animation {
  final double speed = .5; /* expect: double-literal-format */
  final double delay = 1.0; /* expect: double-literal-format */
  final double curve = .25; /* expect: double-literal-format */
}

double calculateOpacity() {
  return .8; /* expect: double-literal-format */
}

void processValues() {
  final list = [.1, .2, .3]; /* expect: double-literal-format */ /* expect: double-literal-format */ /* expect: double-literal-format */
  final map = {
    'x': .5, /* expect: double-literal-format */
    'y': 1.0, /* expect: double-literal-format */
  };
}

void mathOperations() {
  final result = .5 + .25; /* expect: double-literal-format */ /* expect: double-literal-format */
  final product = 2.0 * .75; /* expect: double-literal-format */ /* expect: double-literal-format */
}

bool validateRange(double value) {
  return value > .0; /* expect: double-literal-format */
}

void mixedFormats() {
  final a = .5; /* expect: double-literal-format */
  final b = 3.0; /* expect: double-literal-format */
  final c = .125; /* expect: double-literal-format */
  final d = 100.0; /* expect: double-literal-format */
}
