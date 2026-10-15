// Declaration-only SDK excerpt for SDK-free semantic tests and corpus tools.
library dart.core;
export 'dart:async' show Future, FutureOr;
class Object {
  external const Object();
  external String toString();
  external int get hashCode;
  external bool operator ==(Object other);
}
abstract class Type {}
abstract class Comparable<T> {
  int compareTo(T other);
}
class DateTime implements Comparable<DateTime> {
  external int compareTo(DateTime other);
}
// Constructor and equality excerpt pinned to Dart SDK 3.12.0:
// https://github.com/dart-lang/sdk/blob/3.12.0/sdk/lib/core/duration.dart
class Duration implements Comparable<Duration> {
  static const int microsecondsPerMillisecond = 1000;
  static const int millisecondsPerSecond = 1000;
  static const int secondsPerMinute = 60;
  static const int minutesPerHour = 60;
  static const int hoursPerDay = 24;
  static const int microsecondsPerSecond =
      microsecondsPerMillisecond * millisecondsPerSecond;
  static const int microsecondsPerMinute =
      microsecondsPerSecond * secondsPerMinute;
  static const int microsecondsPerHour = microsecondsPerMinute * minutesPerHour;
  static const int microsecondsPerDay = microsecondsPerHour * hoursPerDay;
  static const Duration zero = Duration(seconds: 0);
  final int _duration;
  const Duration({
    int days = 0,
    int hours = 0,
    int minutes = 0,
    int seconds = 0,
    int milliseconds = 0,
    int microseconds = 0,
  }) : this._microseconds(
         microseconds +
             microsecondsPerMillisecond * milliseconds +
             microsecondsPerSecond * seconds +
             microsecondsPerMinute * minutes +
             microsecondsPerHour * hours +
             microsecondsPerDay * days,
       );
  const Duration._microseconds(int duration) : _duration = duration + 0;
  int get inMicroseconds => _duration;
  bool operator ==(Object other) =>
      other is Duration && _duration == other.inMicroseconds;
  int get hashCode => _duration.hashCode;
  int compareTo(Duration other) => _duration.compareTo(other._duration);
}
abstract class Iterator<E> {
  E get current;
  bool moveNext();
}
abstract class bool {
  external const factory bool.fromEnvironment(String name, {bool defaultValue = false});
}
abstract class num implements Comparable<num> {}
abstract class int extends num {}
abstract class double extends num {}
abstract class Enum {}
abstract final class Function {}
abstract class String {
  String operator [](int index);
  String operator *(int times);
}
abstract class Iterable<E> {
  bool contains(Object? element);
  bool get isNotEmpty;
  bool any(bool test(E element));
  Iterable<E> where(bool test(E element));
  Iterable<T> map<T>(T toElement(E element));
  List<E> toList({bool growable = true});
  E get first;
  E get last;
  E get single;
  E firstWhere(bool test(E element), {E orElse()?});
}
abstract class List<E> implements Iterable<E> {
  E operator [](int index);
  int indexOf(E element, [int start = 0]);
  bool remove(Object? value);
}
abstract class Set<E> implements Iterable<E> {
  bool remove(Object? value);
  E? lookup(Object? object);
}
abstract class Map<K, V> {
  V? operator [](Object? key);
  bool containsKey(Object? key);
  bool containsValue(Object? value);
  V? remove(Object? key);
  Iterable<V> get values;
}
class MapEntry<K, V> {
  final K key;
  final V value;
  const MapEntry(this.key, this.value);
}
external void print(Object? object);
external bool identical(Object? a, Object? b);
