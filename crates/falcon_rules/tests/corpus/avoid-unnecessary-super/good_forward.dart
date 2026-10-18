class Base {
  int get x => 1;
}
final class ForwardShadows extends Base {
  void variable() {
    print(super.x);
    var x = 1;
  }
  void function() {
    print(super.x);
    int x() => 1;
  }
  void pattern() {
    print(super.x);
    final (x, other) = (1, 2);
  }
  void nested() {
    void read() { print(super.x); }
    var x = 1;
    read();
  }
}
