int? counter;

String? describe(int? value) => value?.toString();

void takeNullable(String? text) {}

int? lastLength;

class Holder {
  Holder(this.name);
  String? name;
  String? copy;
}

int? doubled(int? x) => x!; /* expect: unnecessary-null-checks */

List<String>? first(List<String>? items) {
  return items!; /* expect: unnecessary-null-checks */
}

void main(String? text, int? number, Holder holder, (int?, int) pair) {
  takeNullable(text!); /* expect: unnecessary-null-checks */
  int? copy = number!; /* expect: unnecessary-null-checks */
  holder.copy = text!; /* expect: unnecessary-null-checks */
  counter = number!; /* expect: unnecessary-null-checks */
  final list = <int?>[number!]; /* expect: unnecessary-null-checks */
  final map = <String?, int?>{text!: 1}; /* expect: unnecessary-null-checks */
  int? a;
  int b;
  (a!, b) = pair; /* expect: unnecessary-null-checks */
  print([copy, list, map, a, b, describe(1), doubled(2), first(null)]);
}
