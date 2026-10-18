void casts(String text, int number, List<int> numbers, Set<int> unique, Map<String, int> map) {
  print(text as int); /* expect: avoid-unrelated-type-casts */
  print(number as String); /* expect: avoid-unrelated-type-casts */
  print(numbers as List<String>); /* expect: avoid-unrelated-type-casts */
  print(numbers.cast<String>()); /* expect: avoid-unrelated-type-casts */
  print(unique.cast<String>()); /* expect: avoid-unrelated-type-casts */
  print(map.cast<int, int>()); /* expect: avoid-unrelated-type-casts */
  print(map.cast<String, String>()); /* expect: avoid-unrelated-type-casts */
}

class Animal {}
class NotAnimal {}
void unrelatedClasses(Animal animal) {
  print(animal as NotAnimal); /* expect: avoid-unrelated-type-casts */
}
void castPattern(String text) {
  if (text case final number as int) print(number); /* expect: avoid-unrelated-type-casts */
}
