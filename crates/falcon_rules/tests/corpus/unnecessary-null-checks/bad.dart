int? counter;

String? describe(int? value) => value?.toString();

void takeNullable(String? text) {}

int? lastLength;

class Holder {
  Holder(this.name);
  String? name;
  String? copy;
  String? _cache;
  late final String? _late;

  // Fields are never promotable on assignment, private or `late` alike.
  void fill(String? value) {
    _cache = value!; /* expect: unnecessary-null-checks */
    _late = value!; /* expect: unnecessary-null-checks */
    copy = value!; /* expect: unnecessary-null-checks */
  }
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

class UriContext {
  String fromUri(Object? uri) => '$uri';
}

final UriContext uriContext = UriContext();

// A top-level variable receiver resolves to its declared type.
String fromUri(Object? uri) => uriContext.fromUri(uri!); /* expect: unnecessary-null-checks */

// Analyzer 3.13.5: adv-AdvA/repro/r14_unnecessary_null_checks.dart.
void takesDynamic(dynamic value) {}
class NullableOperators {
  void operator []=(int index, int? value) {}
  NullableOperators operator +(int? value) => this;
}
dynamic returnsDynamic(int? value) => value!; /* expect: unnecessary-null-checks */
void nullableContexts(int? value, NullableOperators receiver, Map<String, int?> map) {
  takesDynamic(value!); /* expect: unnecessary-null-checks */
  receiver[0] = value!; /* expect: unnecessary-null-checks */
  receiver + value!; /* expect: unnecessary-null-checks */
  map['key'] = value!; /* expect: unnecessary-null-checks */
}
Future<int?> nullableAsync(int? value) async => value!; /* expect: unnecessary-null-checks */
Stream<int?> nullableStream(int? value) async* {
  yield value!; /* expect: unnecessary-null-checks */
}
// Official test_yieldSyncStar_iterableOfNullable and returnAsync_*_await.
Iterable<int?> nullableIterable(int? value) sync* {
  yield value!; /* expect: unnecessary-null-checks */
}
Future<int?> nullableAwait(int? value) async => await value!; /* expect: unnecessary-null-checks */
int? get nullableGetter => getterValue!; /* expect: unnecessary-null-checks */
int? getterValue;
