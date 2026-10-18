void loopBound() {
  for (final element in [1, 2, 3]) {
    final doubled = element * 2;
    print(doubled);
  }
}

void mutableBinding() {
  var width = 10;
  for (var i = 0; i < 3; i++) {
    final padded = width + 2;
    print(padded);
    width++;
  }
}

void unwrittenMutableBinding() {
  var width = 10;
  for (var i = 0; i < 3; i++) {
    final padded = width + 2;
    print(padded);
  }
}

void mutableResult() {
  final width = 10;
  for (var i = 0; i < 3; i++) {
    var padded = width + 2;
    print(padded++);
  }
}

void mutatedSource() {
  final list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
    list.add(i);
  }
}

void mutatedLength() {
  final list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
    list.length--;
  }
}

void escapingSource() {
  final list = [1, 2, 3];
  final alias = list;
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
    alias.add(i);
  }
}

void escapingView(List<Iterable<int>> results) {
  final list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    results.add(reversed);
  }
}

void freshIdentity(List<List<int>> results) {
  for (var i = 0; i < 3; i++) {
    final list = <int>[];
    results.add(list);
  }
}

void sideEffects(int Function() next) {
  for (var i = 0; i < 3; i++) {
    final value = next();
    print(value);
  }
}

void closures(List<int Function()> callbacks) {
  final width = 10;
  for (var i = 0; i < 3; i++) {
    final callback = () => width + i;
    callbacks.add(callback);
  }
}

void capturedSource(List<void Function()> callbacks) {
  final list = [1, 2, 3];
  callbacks.add(() => list.add(4));
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
  }
}

void unknownCollection(List<int> list) {
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
  }
}

void possibleThrow() {
  final numerator = 10;
  final denominator = 0;
  for (var i = 0; i < 3; i++) {
    final quotient = numerator ~/ denominator;
    print(quotient);
  }
}

void conditionalDeclaration(bool enabled) {
  final width = 10;
  for (var i = 0; i < 3; i++) {
    if (enabled) {
      final padded = width + 2;
      print(padded);
    }
  }
}

class Custom {
  Custom operator +(Custom other) => Custom();
  Iterable<int> get reversed => [DateTime.now().microsecond];
}

void customOperations() {
  final value = Custom();
  for (var i = 0; i < 3; i++) {
    final sum = value + value;
    final reversed = value.reversed;
    print(sum);
    print(reversed.length);
  }
}

void alreadyHoisted() {
  final list = [1, 2, 3];
  final reversed = list.reversed;
  for (final element in list) {
    print(reversed.length + element);
  }
}

void promotedNullable(int? input) {
  final width = input;
  for (var i = 0; i < 3; i++) {
    if (width == null) continue;
    final padded = width + 1;
    print(padded);
  }
}

void promotedCollection() {
  final Object list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    if (list is! List<int>) continue;
    final reversed = list.reversed;
    print(reversed.length);
  }
}

void literalArithmetic() {
  for (var i = 0; i < 3; i++) {
    final value = (2 + 3) * 4;
    print(value);
  }
}

void indexMutation() {
  final list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
    list[0] = i;
  }
}

void passingSource(void Function(List<int>) mutate) {
  final list = [1, 2, 3];
  for (var i = 0; i < 3; i++) {
    final reversed = list.reversed;
    print(reversed.length);
    mutate(list);
  }
}
