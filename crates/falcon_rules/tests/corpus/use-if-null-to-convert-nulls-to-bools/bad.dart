bool? maybe() => null;

class Flags {
  bool? enabled;
}

void check(bool? flag, Flags flags) {
  if (flag == true) {} /* expect: use-if-null-to-convert-nulls-to-bools */
  if (flag != false) {} /* expect: use-if-null-to-convert-nulls-to-bools */
  print(maybe() == true); /* expect: use-if-null-to-convert-nulls-to-bools */
  print(flags.enabled != false); /* expect: use-if-null-to-convert-nulls-to-bools */
  final bool? local = flag;
  print(local == true); /* expect: use-if-null-to-convert-nulls-to-bools */
}

Future<bool?> answer() async => null;

Future<void> awaited() async {
  print((await answer()) == true); /* expect: use-if-null-to-convert-nulls-to-bools */
  final result = await answer();
  print(result != false); /* expect: use-if-null-to-convert-nulls-to-bools */
}
