void conditions(bool ready, bool other, int count, int limit, double floating,
    dynamic unknown, String name) {
  if (ready && !other) {}
  if (count == 3 || count == 4) {}
  if (count < 4 && count > 2) {}
  if (count <= 4 && count >= 4) {}
  if (count < 4 || count > 4) {}
  if (count >= 3 && count != 3) {}
  if (count < 4 && limit > 4) {}
  if (name == 'a b' && name != 'ab') {}
  // NaN makes both sides false, so this is not a tautology.
  if (floating < 4 || floating >= 4) {}
  if (unknown == 3 && unknown == 4) {}
  if (next() && !next()) {}
  if (changing && !changing) {}
  if (nextNumber() == 3 && nextNumber() != 3) {}
}

bool next() => DateTime.now().millisecondsSinceEpoch.isEven;
int nextNumber() => DateTime.now().millisecondsSinceEpoch;
bool get changing => next();

class Custom {
  bool operator <(int other) => next();
  bool operator >(int other) => next();
  @override
  bool operator ==(Object other) => next();
  @override
  int get hashCode => 0;
}

void overloaded(Custom value) {
  if (value < 4 && value > 4) {}
  if (value == 3 && value != 3) {}
}

class Source {
  bool get ready => next();
  int get count => nextNumber();
  void reads() {
    if (ready && !ready) {}
    if (count == 3 && count != 3) {}
  }
}

class StoredValues {
  final bool ready = true;
  final int count = 0;

  void reads() {
    if (ready && !ready) {}
    if (count == 3 && count != 3) {}
    if (count > 4 && count < 4) {}
  }
}

class ChangingValues extends StoredValues {
  @override
  bool get ready => next();
  @override
  int get count => nextNumber();
}

bool globalReady = true;
int globalCount = 0;
void globals() {
  if (globalReady && !globalReady) {}
  if (globalCount > 4 && globalCount < 4) {}
}
