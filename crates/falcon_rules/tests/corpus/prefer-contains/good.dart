bool present(List<String> values, String value) => values.contains(value);
class Search { int indexOf(Object value) => 0; }
bool custom(Search value) => value.indexOf('x') == -1;
bool listStart(List<String> values, String value) => values.indexOf(value, 1) >= 0;
bool stringNonzeroStart(String text, String value) => text.indexOf(value, 2) >= 0;

const minusOneDouble = -1.0;
bool doubleConstant(String text) => text.indexOf('x') == minusOneDouble;
bool assigned(String text) {
  int index;
  return (index = text.indexOf('x')) != -1;
}
bool positive(String text) => text.indexOf('x') > 0;

class List<E> {
  int indexOf(E value) => 0;
}
bool ambiguousList(List<String> values, String value) => values.indexOf(value) >= 0;
