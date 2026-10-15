// SDK-free equivalents of official generic alias and object-pattern cases.
class Cell<T> {
  final T value;
  const Cell(this.value);
  const Cell.named(this.value);
}
typedef CellAlias<T> = Cell<T>;
typedef FixedCell = Cell<int>;
mixin SequenceMixin<T> {}
class RawSequence with SequenceMixin {} /* expect: always-specify-types */

CellAlias rawAlias = throw ''; /* expect: always-specify-types */
CellAlias<int> inferredAlias = CellAlias(1); /* expect: always-specify-types */
CellAlias<int> namedAlias = CellAlias.named(1); /* expect: always-specify-types */
CellAlias<int> constantAlias = const CellAlias.named(1); /* expect: always-specify-types */

void patterns(Object value) {
  if (value case Cell(value: int item)) {} /* expect: always-specify-types */
  if (value case CellAlias(value: int item)) {} /* expect: always-specify-types */
  if (value case Cell<int>(value: final item)) {} /* expect: always-specify-types */
}

typedef void Untyped(int); /* expect: always-specify-types */
typedef String Modified(final value); /* expect: always-specify-types */
