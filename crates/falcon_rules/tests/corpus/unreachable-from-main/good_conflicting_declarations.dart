// A valid executable library: every declaration is reachable.
void main() {
  Twice().render();
  unrelated();
}

class Twice {
  void render() {}
}

void unrelated() {}
