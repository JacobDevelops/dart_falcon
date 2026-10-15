// Tear-offs, static accesses, `is` checks, and shadowed type names.
import 'dart:async';

extension type Plain(int value) {}
class Cell<T> {
  Cell(this.value);
  Cell.named(this.value);
  T value;
  static int count = 0;
  static Cell<int> make() => Cell<int>(1);
}

void use(Object? value) {}

void references(Object value) {
  use(Cell<int>);
  use(Cell.new);
  use(Cell.named);
  use(Cell.count);
  use(Cell.make());
  use(value is Cell);
  use(value is FutureOr);
  final Plain plain = Plain(1);
  use(plain);
}

void shadowed() {
  final Type Cell = int;
  use(Cell);
}

class Shadow {
  int get Cell => 1;
  void read() => use(Cell);
}

void assign((int, int) record) {
  int first;
  int second;
  (first, second) = record;
  use(first + second);
}

class TypeParameterShadow<Cell> {
  Type read() => Cell;
  Cell? field;
  Type method<Wrapped>() => Wrapped;
}
Type shadowFunction<Cell>() => Cell;
void shadowClosure() {
  final Type Function<Cell>() read = <Cell>() => Cell;
  use(read);
}
typedef Mapper<Cell> = Cell Function(Cell value);
typedef Cell LegacyMapper<Cell>(Cell value);
mixin Tagged<T> {}
class Applied<Cell> = Pair<Cell> with Tagged<Cell>;
class Pair<T> {
  Pair(this.first);
  T first;
}
Type Function<Cell>(Cell value) genericFunction = <T>(T value) => T;
void commentedAccess() {
  use(Cell/*(x)*/.count);
}
