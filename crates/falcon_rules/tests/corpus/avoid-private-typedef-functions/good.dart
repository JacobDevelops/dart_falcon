// Public typedefs are not checked.
typedef Callback = void Function(int);

// Used more than once.
typedef _Handler = void Function(String);

void on(_Handler handler) {}
void off(_Handler handler) {}

// Generic aliases are not reported.
typedef _Mapper<T> = T Function(T);

// Aliases of non-function types are not reported.
typedef _Ints = List<int>;
typedef _Pair = (int, int);

// Legacy typedef used twice.
typedef void _Listener(int event);

final List<_Listener> listeners = <_Listener>[];

void main() {
  _Mapper<int>? mapper;
  _Ints? ints;
  _Pair? pair;
  print('$mapper $ints $pair');
}
