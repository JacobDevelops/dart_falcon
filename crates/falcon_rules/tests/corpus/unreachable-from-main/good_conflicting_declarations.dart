// A library that declares the same top-level name twice does not compile;
// reachability through the ambiguous name is unknowable, so nothing is
// reported.

void main() {
  Twice();
}

class Twice {
  void render() {}
}

class Twice {
  void render() {}
}

void unrelated() {}
