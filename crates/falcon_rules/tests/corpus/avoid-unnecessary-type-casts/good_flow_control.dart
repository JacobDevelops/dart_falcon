// `break outer;` reaches the end of the labeled block with `x` still nullable.
void labeledBreak(int? x) {
  outer:
  {
    if (x == null) break outer;
  }
  print(x as int);
}

// A closure registered in the loop body may write `x` after the loop.
void loopExitCapture(int? x, bool Function() c, void Function(void Function()) reg) {
  while (c()) {
    reg(() {
      x = null;
    });
  }
  if (x != null) print(x as int);
}

// A closure created in the `try` may have written `x` before the `catch`.
void catchCapture(int? x, void Function(void Function()) run) {
  try {
    run(() {
      x = null;
    });
  } catch (e) {
    if (x != null) print(x as int);
  }
}

// The `finally` sees the write of the `try` body.
void finallyWrite(int? x, int? y) {
  if (x == null) return;
  try {
    x = y;
  } finally {
    print(x as int);
  }
}

// A switch case's write reaches the code after the switch.
void switchWrite(int? x, int? y, int k) {
  if (x == null) return;
  switch (k) {
    case 0:
      x = y;
  }
  print(x as int);
}
