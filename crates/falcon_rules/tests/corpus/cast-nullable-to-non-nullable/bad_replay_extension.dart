extension type Nullable(Object? value) {}
extension type NonNullable(Object? value) implements Nullable {}
void f() {
  null as NonNullable; /* expect: cast-nullable-to-non-nullable */
  null as Nullable;
  null as NonNullable?;
}
