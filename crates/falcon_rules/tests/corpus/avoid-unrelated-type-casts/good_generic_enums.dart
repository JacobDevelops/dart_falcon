abstract interface class I<T> {}
enum E<T> implements I<T> { a<Never>(), b<int>() }
mixin M<T> implements I<T> {}
enum Mixed<T> with M<T> { a<Never>(), b<int>() }

I<String> direct(E<int> value) => value as I<String>;
I<String> mixed(Mixed<int> value) => value as I<String>;
I<int> concrete(E<int> value) => value as I<int>;
I<int> concreteMixed(Mixed<int> value) => value as I<int>;
I<String> raw(E value) => value as I<String>;
I<String> rawMixed(Mixed value) => value as I<String>;
