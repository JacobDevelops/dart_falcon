var shared = 0; /* expect: avoid-top-level-member-access */

class PrimaryLocal(int value) {
  this {
    final shared = value;
    print(shared); /* expect: avoid-top-level-member-access */
    {
      final shared = value;
      print(shared);
    }
  }
}

class PrimaryFunction(int value) {
  this {
    print(shared()); /* expect: avoid-top-level-member-access */
    int shared() => value;
  }
}
