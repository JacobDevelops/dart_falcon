// Dart 3.13.5 flow analysis keeps these promotions, so no receiver is dynamic.
class Animal {
  String get name => '';
}

class Dog extends Animal {
  bool get good => true;
  void bark() {}
}

// A join keeps the promotions both paths share: `d` stays `Animal`.
void sharedPromotionAfterIf(dynamic d) {
  if (d is! Animal) return;
  if (d is Dog) d.bark();
  print(d.name);
}

void sharedPromotionAfterAnd(dynamic d) {
  if (d is! Animal) return;
  final ok = d is Dog && d.good;
  print(ok);
  print(d.name);
}

// `int + int` is `int`, a type of interest, so the write keeps the promotion.
void compoundAssignmentKeepsPromotion(dynamic d) {
  if (d is int) {
    d += 1;
    print(d.isEven);
  }
}

void incrementKeepsPromotion(dynamic d) {
  if (d is int) {
    d++;
    print(d.isEven);
    --d;
    print(d.isOdd);
  }
}
