// A closure keeps the promotion of a variable its executable never writes.
void closureKeeps(int? x, List<int> xs) {
  if (x == null) return;
  xs.forEach((_) => print(x as int));
}

// A later write may run before the closure does.
void closureLaterWrite(int? x, List<int> xs) {
  if (x == null) return;
  xs.forEach((_) => print(x as int)); /* expect: cast-nullable-to-non-nullable */
  x = null;
}

// `break outer;` reaches the end of the labeled block with `x` still nullable.
void labeledBreak(int? x) {
  outer:
  {
    if (x == null) break outer;
    print(x as int);
  }
  print(x as int); /* expect: cast-nullable-to-non-nullable */
}

// A closure registered in the loop body may write `x` after the loop.
void loopExitCapture(int? x, bool Function() c, void Function(void Function()) reg) {
  while (c()) {
    reg(() {
      x = null;
    });
  }
  if (x != null) print(x as int); /* expect: cast-nullable-to-non-nullable */
}

// A closure created in the `try` may have written `x` before the `catch`.
void catchCapture(int? x, void Function(void Function()) run) {
  try {
    run(() {
      x = null;
    });
  } catch (e) {
    if (x != null) print(x as int); /* expect: cast-nullable-to-non-nullable */
  }
}
