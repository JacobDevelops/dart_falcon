T useMemoized<T>(T Function() valueBuilder, [List<Object?> keys = const []]) => valueBuilder();
T useCallback<T extends Function>(T callback, [List<Object?> keys = const []]) => callback;
void useEffect(dynamic Function() effect, [List<Object?>? keys]) {}
