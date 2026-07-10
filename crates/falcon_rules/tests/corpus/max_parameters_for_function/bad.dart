// Bad: function with 6 parameters exceeds max (default: 5)
void create(String a, String b, int c, double d, bool e, List<String> f) { /* expect: max_parameters_for_function */
  print(a);
  print(b);
  print(c);
  print(d);
  print(e);
  print(f);
}

// Bad: method with 7 parameters
class Example {
  void process(int p1, int p2, int p3, int p4, int p5, int p6, int p7) { /* expect: max_parameters_for_function */
    print(p1);
    print(p2);
  }

  // Another method with 8 parameters
  String buildString(String a, String b, int c, double d, bool e, List<String> f, Map<String, int> g, Set<int> h) { /* expect: max_parameters_for_function */
    return '$a-$b-$c-$d-$e-${f.length}-${g.length}-${h.length}';
  }
}

// Bad: top-level function with 6 parameters
void configure(int x, int y, int z, double scale, bool enabled, String name) { /* expect: max_parameters_for_function */
  print('$x, $y, $z, $scale, $enabled, $name');
}
