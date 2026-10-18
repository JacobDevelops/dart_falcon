void reversedView() {
  final list = [1, 2, 3];
  for (final element in list) {
    final reversed = list.reversed; /* expect: move-variable-outside-iteration */
    for (final value in reversed) {
      print(value + element);
    }
  }
}

void addition() {
  final width = 10;
  for (var i = 0; i < 3; i++) {
    final padded = width + 2; /* expect: move-variable-outside-iteration */
    print(padded + i);
  }
}

void multiplication() {
  final scale = 2.5;
  var remaining = 3;
  while (remaining-- > 0) {
    final doubled = scale * 2; /* expect: move-variable-outside-iteration */
    print(doubled);
  }
}

void subtraction() {
  final maximum = 100;
  var remaining = 3;
  do {
    final limit = maximum - 1; /* expect: move-variable-outside-iteration */
    print(limit);
  } while (remaining-- > 0);
}

void compound() {
  final width = 10;
  final height = 20;
  for (final value in [1, 2]) {
    final area = (width + 2) * (height - 1); /* expect: move-variable-outside-iteration */
    print(area + value);
  }
}

void nested() {
  for (final value in [1, 2]) {
    final size = value + 1;
    for (var i = 0; i < 3; i++) {
      final doubled = size * 2; /* expect: move-variable-outside-iteration */
      print(doubled + i);
    }
  }
}
