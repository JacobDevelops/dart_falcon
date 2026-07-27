// `this.` where no parameter or local shadows the member.
class A {
  int x = 0;
  int y = 0;
  void m() {
    this.x = 1; /* expect: unnecessary-this */
    print(this.y); /* expect: unnecessary-this */
  }
  int get sum => this.x + this.y; /* expect: unnecessary-this */ /* expect: unnecessary-this */
}

class B {
  int value = 0;
  void update(int v) {
    this.helper(); /* expect: unnecessary-this */
    value = v;
  }
  void helper() {}
}

class C {
  int count = 0;
  void inc() {
    this.count += 1; /* expect: unnecessary-this */
  }
}

class SourceOrdered {
  int value = 0;
  void run() {
    print(this.value); /* expect: unnecessary-this */
    final value = 1;
    print(value);
  }
}

class NestedScopes {
  int value = 0;
  void run() {
    {
      final value = 1;
      print(this.value + value);
    }
    print(this.value); /* expect: unnecessary-this */
    (() {
      final value = 2;
      print(this.value + value);
    })();
    print(this.value); /* expect: unnecessary-this */
  }
}

class TryScopes {
  int value = 0;
  void run() {
    try {
      final value = 1;
      print(value);
    } catch (_) {
      print(this.value); /* expect: unnecessary-this */
    } finally {
      print(this.value); /* expect: unnecessary-this */
    }
    print(this.value); /* expect: unnecessary-this */
  }
}
