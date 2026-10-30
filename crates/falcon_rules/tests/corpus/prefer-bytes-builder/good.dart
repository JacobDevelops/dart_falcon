import 'dart:typed_data';

void compose(List<int> bytes, List<String> words, List<num> numbers,
    List<dynamic> unknown, Uint16List wide) {
  words + words;
  words += words;
  words.addAll(words);
  final copy = [...words];
  numbers.addAll(numbers);
  final dynamicCopy = [...unknown];
  final wideCopy = [...wide];
  bytes.add(1);
  bytes[0] += 1;
  const constants = [...<int>[1, 2]];
  final explicitConstants = const [...<int>[1, 2]];
  final builder = BytesBuilder();
  builder.add(bytes);
  builder.toBytes();
}

class Custom {
  void addAll(List<int> bytes) {}
  Custom operator +(List<int> bytes) => this;
}
void unrelated(Custom custom, List<int> bytes) {
  custom.addAll(bytes);
  custom + bytes;
}

void unrelatedChains(List<String> words, List<num> numbers) {
  final wordChain = words + words + words;
  final groupedWords = (words + words) + (words + words);
  final wordSpread = [...(words + words)];
  final numberChain = numbers + (numbers + numbers);
  final numberSpread = [...((numbers + numbers) + numbers)];
  const nestedConstants = [...<int>[1, 2], ...<int>[3, 4]];
  final explicitNestedConstants = const [...<int>[1], ...<int>[2]];
}
