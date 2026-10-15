class Bounded<T extends num> {}
typedef Bound = Bounded<num>;

void defaults({Type value = Bounded}) {}

// Raw generic type literals use instantiate-to-bounds before canonical equality.
void main() {
  defaults(value: Bound); /* expect: avoid-redundant-argument-values */
}
