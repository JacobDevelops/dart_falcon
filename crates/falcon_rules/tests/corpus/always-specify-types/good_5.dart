// Constant-pattern references use the local constant, not the generic type.
class Box<T> {}

bool statement(Object value) {
  const int Box = 1;
  switch (value) {
    case Box:
      return true;
  }
  return false;
}

bool expression(Object value) {
  const int Box = 1;
  return switch (value) {
    Box => true,
    _ => false,
  };
}

bool nested(Object value) {
  const int Box = 1;
  if (value case [Box]) {
    return true;
  }
  return false;
}

bool shadowing(Object value) {
  const int Box = 1;
  {
    const int Box = 2;
    if (value case Box) {
      return true;
    }
  }
  return switch (value) {
    Box => true,
    _ => false,
  };
}
