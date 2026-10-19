VoidCallback? callback;
void takesArgument(void Function(int) callback) {}
void returnsValue(int Function() callback) {}
void generic(void Function<T>() callback) {}
typedef void Legacy();
