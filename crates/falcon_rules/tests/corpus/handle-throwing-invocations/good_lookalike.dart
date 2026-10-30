class Throws {
  const Throws([Set<Type> types = const {}]);
}
class ParseFailure {}
@Throws({ParseFailure})
void unrelated() {}
void caller() { unrelated(); }
