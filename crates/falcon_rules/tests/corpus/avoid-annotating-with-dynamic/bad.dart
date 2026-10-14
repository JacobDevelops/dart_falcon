class Holder {
  dynamic value;
  Holder(dynamic this.value); /* expect: avoid-annotating-with-dynamic */
  void set(dynamic v) {} /* expect: avoid-annotating-with-dynamic */
  set current(dynamic v) {} /* expect: avoid-annotating-with-dynamic */
}

class Child extends Holder {
  Child(dynamic super.value); /* expect: avoid-annotating-with-dynamic */
}

void log(dynamic message) {} /* expect: avoid-annotating-with-dynamic */

void named({dynamic extra}) {} /* expect: avoid-annotating-with-dynamic */

typedef Callback = void Function(dynamic event); /* expect: avoid-annotating-with-dynamic */

void main() {
  final f = (dynamic x) => x; /* expect: avoid-annotating-with-dynamic */
  print(f);
}
