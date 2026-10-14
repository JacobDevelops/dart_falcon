void loops(List<int> numbers, List<(int, String)> rows, List<List<int>> lists) {
  for (final n in numbers) {
    print(n);
  }
  for (var n in numbers) {
    n += 1;
    print(n);
  }
  for (var n in numbers) {
    void bump() {
      n++;
    }

    bump();
  }
  for (final (i, s) in rows) {
    print('$i$s');
  }
  for (var (i, s) in rows) {
    i = 0;
    print('$i$s');
  }
  for (var (i, _) in rows) {
    print(i);
  }
  for (var [first, ...rest] in lists) {
    print('$first$rest');
  }
  var existing = 0;
  for (existing in numbers) {
    print(existing);
  }
  for (var i = 0; i < numbers.length; i++) {
    print(i);
  }
  final copy = [for (final n in numbers) n];
  print(copy);
}

// The analyzer requires an enclosing function body, not a field initializer.
final topLevelCollection = [for (var n in [1, 2]) n];

class Initializer {
  final values = [for (var n in [1, 2]) n];
}
