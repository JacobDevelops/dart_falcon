// Good: function with 3 parameters under limit
void create(String a, String b, int c) {
  print(a);
  print(b);
  print(c);
}

// Good: using a config object instead of many parameters
class Config {
  final String name;
  final int count;
  final double value;
  final bool flag;
  final List<String> items;
}

void configure(Config config) {
  print(config.name);
}

// Good: method with 5 parameters (at limit)
class Example {
  void process(int p1, int p2, int p3, int p4, int p5) {
    print(p1);
  }

  // Good: constructor with 4 parameters
  Example.simple(String name, int age, double height, bool active) {
  }

  // Good: single parameter
  void singleParam(String value) {
    print(value);
  }

  // Good: no parameters
  void noParams() {
    print('Called with no parameters');
  }
}

// Good: optional parameters within limit
void functionalApproach({String? name, int? age, double? score, bool? flag}) {
  print('$name, $age, $score, $flag');
}

// dcl's number-of-parameters metric supports only functions and methods, never
// constructors — so a wide named-parameter constructor is never counted.
class Wide {
  final int a, b, c, d, e, f, g;

  const Wide({
    required this.a,
    required this.b,
    required this.c,
    required this.d,
    required this.e,
    required this.f,
    required this.g,
  });

  // `copyWith` returning the same class is explicitly exempt.
  Wide copyWith({int? a, int? b, int? c, int? d, int? e, int? f, int? g}) {
    return this;
  }
}
