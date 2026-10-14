abstract class Predicate { /* expect: one-member-abstracts */
  bool test(Object value);
}

abstract class Loader { /* expect: one-member-abstracts */
  const Loader();
  Future<String> load(String path);
}

abstract interface class Formatter { /* expect: one-member-abstracts */
  String format(int value);
}

abstract class Adder { /* expect: one-member-abstracts */
  int operator +(int other);
}

abstract mixin class Runner { /* expect: one-member-abstracts */
  void run();
}
