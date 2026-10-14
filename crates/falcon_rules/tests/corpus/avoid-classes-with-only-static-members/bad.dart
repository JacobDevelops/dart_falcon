// Only static methods: move them to the top level.
class DateUtils { /* expect: avoid-classes-with-only-static-members */
  static DateTime mostRecent(List<DateTime> dates) =>
      dates.reduce((a, b) => a.isAfter(b) ? a : b);
}

// Static non-const fields count as instance-free state.
class _Favorites { /* expect: avoid-classes-with-only-static-members */
  static int mostFavorite = 0;
}

// A static getter induces a non-const field.
class Config { /* expect: avoid-classes-with-only-static-members */
  static String get name => 'config';
}

// A default constructor does not make the class a real type.
class Helpers { /* expect: avoid-classes-with-only-static-members */
  Helpers();
  static void run() {}
  static const int retries = 3;
}

// `extends Object` adds no members.
abstract class Registry extends Object { /* expect: avoid-classes-with-only-static-members */
  static final List<String> names = [];
  static void register(String name) => names.add(name);
}

// Static setter plus static method.
class Counter { /* expect: avoid-classes-with-only-static-members */
  static int _count = 0;
  static set count(int value) => _count = value;
  static void reset() => _count = 0;
}
