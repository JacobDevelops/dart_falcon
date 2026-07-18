// `this.` needed because of a shadowing parameter or local, or no `this.`.
class A {
  int x = 0;
  void m(int x) {
    this.x = x;
  }
}

class B {
  int value = 0;
  void update() {
    var value = 5;
    this.value = value;
  }
}

class C {
  int count = 0;
  void inc() {
    count += 1;
  }
  int get doubled => count * 2;
}

class D {
  int total = 0;
  void add(int total) {
    this.total += total;
  }
}

class E {
  void run(int helper) {
    this.helper();
  }
  void helper() {}
}
