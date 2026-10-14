void call(Object a, [Object? b, Object? c]) {}
void named({Object? a, Object? b}) {}
void test(String name, void Function() body) {}

class Point {
  final int x;
  final int y;

  Point(
    this.x,
    this.y,
  ) : assert(x >= 0, 'x must be non-negative');

  Point.origin({
    this.x = 0,
    this.y = 0,
  });
}

void declaration(int a, int b) {}

void f() {
  call('a', 'b');

  // A multi-line block-bodied function literal as the last argument is exempt.
  test('name', () {
    print('body');
  });

  // So is a multi-line string literal ...
  call('a', '''
first line
second line
''');

  // ... and a multi-line collection literal.
  call(1, [
    2,
    3,
  ]);
  call(1, {
    'a': 2,
  });

  final list = [
    1,
    2,
  ];
  final map = {'a': 1, 'b': 2};
  print(map);
  named(
    a: 1,
    b: 2,
  );
  call();
}

final list = <int>[];

typedef Callback = void Function(
  int a,
  int b,
);
