int apply(int Function(int) fn) => fn(1);

void each(List<String> names, void Function(String name) action) {}

void dyn(void Function(dynamic value) action) {}

void untypedContext(Function action) {}

void f(List<String> names) {
  apply((x) => x + 1);
  each(names, (name) {
    print(name);
  });
  dyn((dynamic value) {});
  untypedContext((int x) => x);
  final g = (int x) => x;
  var h = (String s) {};
  print([g, h]);
}
