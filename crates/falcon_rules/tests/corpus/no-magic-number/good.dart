// Good examples for no-magic-number. Every non-allowed literal here sits in a
// position dcl exempts: allow-list, variable/field initializer, collection
// literal, const constructor, const map, DateTime, or index expression.

// Allowed numbers (-1, 0, 1) are never magic, in any position.
int allowed() => compute(0) + compute(1) + compute(-1);

// Variable and field initializers are exempt (VariableDeclaration ancestor).
final topLevelTimeout = 3000;

class Config {
  final int maxRetries = 5;
  static const int port = 8080;

  int scaled(int value) {
    final factor = 100;
    return factor;
  }
}

// Direct elements of a list/set literal are exempt.
List<int> sizes() => [12, 24, 48];

// A const constructor exempts its whole argument subtree.
Widget spacer() => const SizedBox(height: 12, width: 800);

// A const map is exempt, and so is a DateTime constructor.
Map<String, int> durations() => const {'short': 15, 'long': 60};

DateTime epoch() => DateTime(2020, 1, 1);

// A literal used directly as an index is exempt.
int firstFew(List<int> xs) => xs[5];
