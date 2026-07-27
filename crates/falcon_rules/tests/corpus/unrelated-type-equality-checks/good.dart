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
