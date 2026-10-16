class EncodedT {} /* expect: unreachable-from-main */
class KeyT {} /* expect: unreachable-from-main */
class Concrete {}

class Storage<KeyT, EncodedT> {
  Object values() => <KeyT, EncodedT>{};
}

void main() {
  Storage<Concrete, int>().values();
}
