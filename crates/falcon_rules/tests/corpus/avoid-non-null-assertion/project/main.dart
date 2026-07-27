class CustomMap<K, V> implements Map<K, V> {}

V read<K, V>(CustomMap<K, V> values, K key) => values[key]!;
