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
}
