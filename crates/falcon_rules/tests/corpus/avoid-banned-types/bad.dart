class Legacy {}
typedef Alias = Legacy; /* expect: avoid-banned-types */
Legacy? value; /* expect: avoid-banned-types */
List<Legacy> values = []; /* expect: avoid-banned-types */
Alias? alias; /* expect: avoid-banned-types */

class Wrapper<T> {
  const Wrapper();
}
void genericConstructions() {
  final list = new List<Legacy>.empty(); /* expect: avoid-banned-types */
  final wrapper = const Wrapper<Legacy>(); /* expect: avoid-banned-types */
}
