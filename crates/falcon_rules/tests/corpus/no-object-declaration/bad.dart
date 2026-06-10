// Bad: Object type declarations should use a more specific type
void example() {
  Object data = fetchData(); /* expect: no-object-declaration */
  final Object result; /* expect: no-object-declaration */
  Object? nullable = null; /* expect: no-object-declaration */
  var obj = Object(); /* expect: no-object-declaration */
}

class Store {
  Object cache = {}; /* expect: no-object-declaration */

  void process(Object input) { /* expect: no-object-declaration */
    print(input);
  }
}
