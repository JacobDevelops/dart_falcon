// Extension types, `FutureOr`, type literals, and pattern assignments.
import 'dart:async';

extension type Wrapped<T>(T value) {}
class Cell<T> {
  Cell(this.value);
  T value;
}
class Pair<T> {
  Pair(this.first);
  T? first;
}

Wrapped rawExtension = Wrapped<int>(1); /* expect: always-specify-types */
Wrapped<int> constructed = Wrapped(1); /* expect: always-specify-types */
FutureOr futureOr = 1; /* expect: always-specify-types */
FutureOr<int> Function() callback = () => futureOr as FutureOr<int>;
void takesCallback(FutureOr Function() action) {} /* expect: always-specify-types */

void use(Object? value) {}

void literals(Object value) {
  use(Cell); /* expect: always-specify-types */
  use('$Wrapped'); /* expect: always-specify-types */
  use(<Type>[Cell]); /* expect: always-specify-types */
  use(value as FutureOr); /* expect: always-specify-types */
  if (value case Wrapped(value: int item)) { /* expect: always-specify-types */
    use(item);
  }
  switch (value) {
    case Cell: /* expect: always-specify-types */
      break;
  }
}

void assign<T>(T? item, Pair<T> pair) {
  Pair(first: item) = pair; /* expect: always-specify-types */
  use(item);
}

void parenthesized() {
  use((Cell).hashCode); /* expect: always-specify-types */
}
