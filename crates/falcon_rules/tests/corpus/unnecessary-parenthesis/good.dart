class Holder {
  bool Function() e;
  Holder(bool Function()? e) : e = e ?? (() => true);
}

extension NullableString on String? {
  bool get isBlank => this == null || this!.isEmpty;
}

int value() => 1;

void f(int a, int b, bool c, String? s, List<int> list, int? n, Object o, List<int>? maybeList) {
  var x = a + b;
  print(x);
  var y = (a + b) * 2;
  var z = (c ? a : b).isEven;
  var w = (a == b);
  c = (a != b);
  var v = (o as int).isEven;
  var u = !(o is int);
  var t = (List<int>).hashCode;
  var r = (n)!;
  var q = (-a).abs();
  var p = (a++).isEven;
  var m = (s?.length).hashCode;
  var k = (s?.length).toString();
  var j = (s?.trim()).isBlank;
  var i = (list..clear()).length;
  ({1, 2, 3}).forEach(print);
  (switch (a) { _ => 1 });
  print([y, z, w, v, u, t, r, q, p, m, k, j, i]);
  print(c ? (maybeList?[0]) : null);
  print({(maybeList?[0]): 1});
  final items = [...(o as List<int>)];
  print(items);
  list..add(1)..[0] = (a + b);
}

bool g(int a, int b) {
  return (a == b);
}

Iterable<bool> gen(int a, int b) sync* {
  yield (a == b);
}

int h(int a, int b) => a * (b + 1);

void cases(Object o) {
  switch (o) {
    case const (1 + 2):
      break;
  }
}

class Callback {
  bool Function() e;
  Callback() : e = (() => true);
}

// Pattern grouping is distinct from parenthesized expressions.
int wildcardPattern(Object? value) => switch (value) {
  (_) => 1,
};

int nestedWildcardPattern(Object? value) => switch (value) {
  ((/* grouping */ _)) => 1,
};

int recordPattern((int?, int?) value) => switch (value) {
  (null, final int right) => right,
  (final int left, null) => left,
  (_) => 0,
};

void statementPattern(Object? value) {
  if (value case (_)) {}
  switch (value) {
    case (_):
      break;
  }
}

// A pattern initializer has a non-expression declaration parent, unlike an
// ordinary variable initializer.
Future<void> patternInitializers(
  int a,
  int b,
  Object value,
  Future<(int, int)> Function() fetch,
) async {
  var (sum) = (a + b);
  var (cast) = (value as int);
  var (check) = (value is int);
  final (left, right) = (await fetch());
  print([sum, cast, check, left, right]);
}
