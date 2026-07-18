class Base {
  void foo() {}
  void bar(int x) {}
  int get val => 0;
  void baz([int x = 0]) {}
}

const protected = 'protected';

class Derived extends Base {
  @override
  void foo() {
    print('extra');
    super.foo();
  }

  @override
  int get val => super.val + 1;

  @override
  void bar(int x) => super.bar(x + 1);

  @override
  @protected
  void extraAnnotated() => superCall();

  @override
  void baz([int x = 0]) => super.baz(x);

  void notOverride() => helper();
}
