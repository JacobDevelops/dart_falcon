T useMemoized<T>(T Function() valueBuilder, [List<Object?> keys = const []]) => valueBuilder();
T useCallback<T extends Function>(T callback, [List<Object?> keys = const []]) => callback;
