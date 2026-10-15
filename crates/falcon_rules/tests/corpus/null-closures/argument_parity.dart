// Analyzer-confirmed cases from adv-AdvB2/repro/nc_quirks.dart, r17.txt.
// Static matching deliberately uses a class's name, not its library.
class Timer {
  static void run(void Function()? f) {}
}
class Future {
  static void wait(List<int> l, {void Function(int)? cleanUp}) {}
}
void f(List<int> l) {
  Timer.run(null); /* expect: null-closures */
  Future.wait([], cleanUp: null); /* expect: null-closures */
  // Analyzer uses the source-order index even with a preceding named argument.
  l.firstWhere(orElse: () => 1, null);
  l.firstWhere((e) => true,
      orElse: /* expect: null-closures */
          null);
  l.firstWhere((e) => true, orElse: (null));
}
