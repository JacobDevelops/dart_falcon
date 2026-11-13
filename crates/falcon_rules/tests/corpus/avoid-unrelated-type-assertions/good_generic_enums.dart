abstract interface class I<T> {}
enum E<T> implements I<T> { a<Never>(), b<int>() }
mixin M<T> implements I<T> {}
enum Mixed<T> with M<T> { a<Never>(), b<int>() }

bool direct(E<int> value) => value is I<String>;
bool mixed(Mixed<int> value) => value is I<String>;
bool concrete(E<int> value) => value is I<int>;
bool concreteMixed(Mixed<int> value) => value is I<int>;
bool raw(E value) => value is I<String>;
bool rawMixed(Mixed value) => value is I<String>;
