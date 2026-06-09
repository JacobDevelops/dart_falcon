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
}
