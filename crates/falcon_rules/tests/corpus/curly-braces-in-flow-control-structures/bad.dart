void f1(bool x) {
  while (x) doThing(); /* expect: curly-braces-in-flow-control-structures */
}

void f2() {
  for (var i = 0; i < 3; i++) doThing(); /* expect: curly-braces-in-flow-control-structures */
}

void f3(bool x) {
  do doThing(); /* expect: curly-braces-in-flow-control-structures */
  while (x);
}

void f4(bool a) {
  if (a)
    doThing(); /* expect: curly-braces-in-flow-control-structures */
}

void f5(bool a) {
  if (a)
    doThing(); /* expect: curly-braces-in-flow-control-structures */
  else
    doOther(); /* expect: curly-braces-in-flow-control-structures */
}

void f6(bool a, List<int> xs) {
  for (final x in xs) print(x); /* expect: curly-braces-in-flow-control-structures */
}

// Closure argument of a constructor invocation (`Expr::New`).
Widget f7(bool x) {
  return new ElevatedButton(onPressed: () {
    while (x) doThing(); /* expect: curly-braces-in-flow-control-structures */
  });
}

// Closure nested inside a collection literal.
List<VoidCallback> f8(bool x) {
  return [
    () {
      if (x)
        doThing(); /* expect: curly-braces-in-flow-control-structures */
    },
  ];
}
