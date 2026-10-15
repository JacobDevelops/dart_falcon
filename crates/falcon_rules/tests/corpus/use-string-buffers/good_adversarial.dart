// Analyzer-confirmed AdvB4: fields have distinct getter and setter elements.
String top = '';

class C {
  String field = '';
  void m(List<String> xs) {
    for (var x in xs) {
      field = field + x;
      top = top + x;
    }
  }
}

class D {
  static String sf = '';
  void m(List<String> xs) {
    for (var x in xs) {
      sf = sf + x;
    }
  }
}
