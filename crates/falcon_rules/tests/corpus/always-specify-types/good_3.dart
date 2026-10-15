class Cell<T> {
  final T value;
  const Cell(this.value);
}
typedef CellAlias<T> = Cell<T>;
typedef FixedCell = Cell<int>;
CellAlias<int> alias = CellAlias<int>(1);
FixedCell fixed = FixedCell(1);

extension type Wrapped<T>(T value) {}
Wrapped<int> typedExtension = Wrapped<int>(1);

void patterns(Object value) {
  if (value is Cell) {}
  if (value case Cell<int>(value: int item)) {}
  if (value case Wrapped<int>(value: int item)) {}
}

typedef void Typed(int value);
typedef void Explicit(dynamic int);
typedef void Wildcard(_);
class WildcardOwner<_> {
  WildcardOwner(_);
}
