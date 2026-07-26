class Alpha {}
class Beta {}

int makeNumber() => 1;

void bad(
  int number,
  String text,
  Alpha alpha,
  Beta beta,
  String value,
  List<int> ints,
  List<String> strings,
  int Function(int) intFn,
  String Function(String) stringFn,
  (int,) intRecord,
  (String,) stringRecord,
) {
  number == text; /* expect: unrelated-type-equality-checks */
  text != true; /* expect: unrelated-type-equality-checks */
  alpha == beta; /* expect: unrelated-type-equality-checks */
  makeNumber() == text; /* expect: unrelated-type-equality-checks */
  ints == strings; /* expect: unrelated-type-equality-checks */
  intFn == stringFn; /* expect: unrelated-type-equality-checks */
  intRecord == stringRecord; /* expect: unrelated-type-equality-checks */
  if (value case == 1) { /* expect: unrelated-type-equality-checks */
    print(value);
  }
}

void scopedEquality(int number) {
  (() {
    final text = 'x';
    number == text; /* expect: unrelated-type-equality-checks */
  })();
  for (final text in <String>['x']) {
    number == text; /* expect: unrelated-type-equality-checks */
  }
  number == 1;
}

class TypedBox {
  TypedBox(this.value);
  final int value;
}

void patternBindings(
  (int,) record,
  List<int> list,
  Map<String, int> map,
  TypedBox box,
) {
  if (record case (var item,)) {
    item == 'x'; /* expect: unrelated-type-equality-checks */
  }
  if (list case [var item]) {
    item == 'x'; /* expect: unrelated-type-equality-checks */
  }
  if (map case {'x': var item}) {
    item == 'x'; /* expect: unrelated-type-equality-checks */
  }
  if (box case TypedBox(value: var item)) {
    item == 'x'; /* expect: unrelated-type-equality-checks */
  }
}
