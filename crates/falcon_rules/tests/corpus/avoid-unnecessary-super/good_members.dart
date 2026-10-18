class Base {
  int get x => 1;
  set x(int value) {}
  void action() {}
}
class Override extends Base {
  @override
  int get x => 2;
  @override
  set x(int value) {}
  @override
  void action() { print('own'); }
  int read() => super.x;
  void write(int value) { super.x = value; }
  void run() { super.action(); }
}
mixin Modified on Base {
  @override
  void action() { print('mixin'); }
}
class WithMixin extends Base with Modified {
  @override
  void action() { super.action(); print('own'); }
}
class Unknown extends ExternalBase {
  int read() => super.x;
}

final class Shadowed extends Base {
  int read(int x) => super.x;
  int local() {
    final x = 42;
    return super.x;
  }
  int closure() {
    int nested(int x) => super.x;
    return nested(1);
  }
}

class Open extends Base {
  int inherited() => super.x;
}
class OpenDescendant extends Open {
  @override
  int get x => 42;
}

final class LocallyExtended extends Base {
  int inherited() => super.x;
}
final class LocalDescendant extends LocallyExtended {
  @override
  int get x => 42;
}
