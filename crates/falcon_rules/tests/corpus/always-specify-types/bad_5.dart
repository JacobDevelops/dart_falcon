// The declaration needs a type, but its constant-pattern use is not a type literal.
class Box<T> {}

bool statement(Object value) {
  const Box = 1; /* expect: always-specify-types */
  switch (value) {
    case Box:
      return true;
  }
  return false;
}
