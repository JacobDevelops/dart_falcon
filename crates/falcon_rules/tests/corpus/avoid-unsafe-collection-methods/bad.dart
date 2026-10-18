import 'package:collection/collection.dart';

void access(List<int> values, Set<String> names, Iterable<int> items, int index) {
  print(values.first); /* expect: avoid-unsafe-collection-methods */
  print(names.last); /* expect: avoid-unsafe-collection-methods */
  print(items.single); /* expect: avoid-unsafe-collection-methods */
  print(values[index]); /* expect: avoid-unsafe-collection-methods */
  print(items.elementAt(index)); /* expect: avoid-unsafe-collection-methods */
  print(values.firstWhere((value) => value > 0)); /* expect: avoid-unsafe-collection-methods */
  print(values.lastWhere((value) => value > 0)); /* expect: avoid-unsafe-collection-methods */
  print(values.singleWhere((value) => value > 0)); /* expect: avoid-unsafe-collection-methods */
  values..first; /* expect: avoid-unsafe-collection-methods */
  values..firstWhere((value) => value > 0); /* expect: avoid-unsafe-collection-methods */
}

void objectPatterns(Object value) {
  if (value case List<int>(first: final first)) { /* expect: avoid-unsafe-collection-methods */
    print(first);
  }
  if (value case Iterable<int>(last: final last)) { /* expect: avoid-unsafe-collection-methods */
    print(last);
  }
  if (value case Set<int>(single: final single)) { /* expect: avoid-unsafe-collection-methods */
    print(single);
  }
}

void packageExtrema(Iterable<int> values) {
  print(values.max); /* expect: avoid-unsafe-collection-methods */
  print(values.min); /* expect: avoid-unsafe-collection-methods */
  values..max; /* expect: avoid-unsafe-collection-methods */
  if (values case Iterable<int>(min: final smallest)) { /* expect: avoid-unsafe-collection-methods */
    print(smallest);
  }
}

class NotString {
  List<int> split(String separator) => [];
}

void customSplit(NotString value) {
  print(value.split(',').first); /* expect: avoid-unsafe-collection-methods */
  print(value.split(',').last); /* expect: avoid-unsafe-collection-methods */
}

void mutatedSplit(String text) {
  text.split(',')..clear()..first; /* expect: avoid-unsafe-collection-methods */
}

void splitResults(String text, Pattern separator) {
  print(text.split(separator).first); /* expect: avoid-unsafe-collection-methods */
  print(text.split(separator).last); /* expect: avoid-unsafe-collection-methods */
  text.split(separator)..first..last; /* expect: avoid-unsafe-collection-methods */ /* expect: avoid-unsafe-collection-methods */
  print(''.split('').first); /* expect: avoid-unsafe-collection-methods */
  print(''.split(RegExp('')).last); /* expect: avoid-unsafe-collection-methods */
}
