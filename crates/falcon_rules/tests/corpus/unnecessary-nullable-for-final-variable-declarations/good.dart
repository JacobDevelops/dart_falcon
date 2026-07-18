final int a = 3;
final int? nullable = maybeNull();
var x = 5;
int? topLevel = 3;

class C {
  final int? field = 3;
  int? notFinal;

  void method() {
    final int? fromCall = compute();
    final int? explicit = null;
    int? mutable = 3;
    print('$fromCall $explicit $mutable');
  }
}

int? maybeNull() => null;
int? compute() => null;
