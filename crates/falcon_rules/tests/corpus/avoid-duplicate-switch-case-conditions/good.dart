bool next() => true;
int f(Object value, bool ready, int count) => switch (value) {
  int() when ready => 1,
  String() when ready => 2,
  int() when !ready => 3,
  double() when count > 1 => 4,
  double() when count > 2 => 5,
  bool() when next() => 6,
  bool() when next() => 7,
  _ => 0,
};
int g(String value) => switch (value) {
  'a b' => 1,
  'ab' => 2,
  _ => 3,
};

bool get changing => next();
const code = 1;

int getterGuard(Object value) => switch (value) {
  int() when changing => 1,
  int() when changing => 2,
  _ => 0,
};

class Flags {
  bool get ready => next();
}

int customGetter(Object value, Flags flags) => switch (value) {
  int() when flags.ready => 1,
  int() when flags.ready => 2,
  _ => 0,
};

// Constant cases are owned by no-duplicate-case-values, even with guards.
int literalCases(int value, bool ready) => switch (value) {
  1 when ready => 1,
  1 when ready => 2,
  code when !ready => 3,
  code when !ready => 4,
  2 || 3 when ready => 5,
  2 || 3 when ready => 6,
  (4) when ready => 7,
  (4) when ready => 8,
  _ => 0,
};

int dynamicGuard(Object value, dynamic unknown) => switch (value) {
  int() when unknown == 1 => 1,
  int() when unknown == 1 => 2,
  _ => 0,
};

class StoredGuard {
  final bool ready = true;
  final String text = '';

  int guard(Object value) => switch (value) {
    int() when ready => 1,
    int() when ready => 2,
    String() when text.isEmpty => 3,
    String() when text.isEmpty => 4,
    _ => 0,
  };
}

class ChangingGuard extends StoredGuard {
  @override
  bool get ready => next();
  @override
  String get text => next() ? '' : 'changing';
}

bool globalReady = true;
int globalGuard(Object value) => switch (value) {
  int() when globalReady => 1,
  int() when globalReady => 2,
  _ => 0,
};
