class Base {}
class Child extends Base {}

void good(
  int integer,
  double decimal,
  num number,
  String? nullableText,
  dynamic anything,
  Base base,
  Child child,
  Never bottom,
  List<int> values,
  List<int>? nullableValues,
) {
  integer == decimal;
  decimal != number;
  nullableText == null;
  anything == integer;
  base == child;
  bottom == 'text';
  values == nullableValues;
  if (nullableText case == null) {
    print(nullableText);
  }
}

class Sibling extends Base {}

class Box<T> {}

class Callable {
  int call() => 1;
}

void related(
  Child child,
  Sibling sibling,
  int Function(int) intFunction,
  String Function(String) stringFunction,
  int Function() function,
  Callable callable,
  Function anyFunction,
  Comparable<num> comparable,
  int integer,
  String text,
  Pattern pattern,
  (int,) record,
  Record anyRecord,
  Box<int> ints,
  Box<num> numbers,
) {
  child == sibling;
  intFunction == stringFunction;
  function == callable;
  function == anyFunction;
  integer == comparable;
  text == pattern;
  record == anyRecord;
  ints == numbers;
}

void typeParameters<T, N extends num>(T value, N number, String text) {
  value == text;
  number == text;
}
