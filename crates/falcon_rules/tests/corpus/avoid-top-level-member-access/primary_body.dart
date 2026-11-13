var shared = 0; /* expect: avoid-top-level-member-access */

class PrimaryLocal(int value) {
  this {
    final shared = value;
    print(shared);
    {
      final shared = value;
      print(shared);
    }
  }
}

class PrimaryFunction(int value) {
  this {
    print(shared());
    int shared() => value;
  }
}
