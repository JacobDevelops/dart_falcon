class Pair {
  final int a;
  final int b;
  Pair(this.a, this.b);
}

void loops(List<int> numbers, List<(int, String)> rows, List<Pair> pairs, List<List<int>> lists,
    List<Map<String, int>> maps) {
  for (var n in numbers) { /* expect: prefer-final-in-for-each */
    print(n);
  }
  for (int n in numbers) { /* expect: prefer-final-in-for-each */
    print(n);
  }
  for (var (i, s) in rows) { /* expect: prefer-final-in-for-each */
    print('$i$s');
  }
  for (var Pair(a: x, b: y) in pairs) { /* expect: prefer-final-in-for-each */
    print(x + y);
  }
  for (var [first, second] in lists) { /* expect: prefer-final-in-for-each */
    print(first + second);
  }
  for (var {'k': value} in maps) { /* expect: prefer-final-in-for-each */
    print(value);
  }
  final doubled = [for (var n in numbers) n * 2]; /* expect: prefer-final-in-for-each */
  print(doubled);
}

Future<void> streams(Stream<int> values) async {
  await for (var v in values) { /* expect: prefer-final-in-for-each */
    print(v);
  }
}
