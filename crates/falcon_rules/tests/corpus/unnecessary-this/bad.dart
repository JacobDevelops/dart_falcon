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
