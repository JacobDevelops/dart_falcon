// Good: function under 100 lines
void shortFunction() {
  print('Line 2');
  print('Line 3');
  print('Line 4');
  print('Line 5');
  print('Line 6');
  print('Line 7');
  print('Line 8');
  print('Line 9');
  print('Line 10');
}

// Good: another function within limits
void anotherFunction() {
  final x = 1;
  final y = 2;
}

// Good: method within 100-line limit
class WellSizedClass {
  void reasonablyLongMethod() {
    print('Line 1');
    print('Line 2');
    print('Line 3');
    print('Line 4');
    print('Line 5');
    print('Line 6');
    print('Line 7');
    print('Line 8');
    print('Line 9');
    print('Line 10');
    print('Line 11');
    print('Line 12');
    print('Line 13');
    print('Line 14');
    print('Line 15');
    print('Line 16');
    print('Line 17');
    print('Line 18');
    print('Line 19');
    print('Line 20');
  }

  int calculate(int a, int b) {
    return a + b;
  }

  void processData() {
    final items = <int>[1, 2, 3];
    for (final item in items) {
      print(item);
    }
  }
}

// Good: getter within limits
class Property {
  int get value {
    return 42;
  }
}
