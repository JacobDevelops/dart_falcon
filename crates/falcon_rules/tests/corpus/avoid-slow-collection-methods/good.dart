import 'dart:async';

bool positive(int value) => value > 0;

class Query {
  Query where(bool Function(int) predicate) => this;
  int get first => 0;
  int get last => 0;
  int get single => 0;
}

abstract class CustomIterable implements Iterable<int> {
  @override
  Iterable<int> where(bool Function(int) predicate) => <int>[1];
}

void search(Iterable<int> values, Iterable<Object> objects,
    Iterable<int>? nullable, dynamic unknown, Query query, CustomIterable custom) {
  values.firstWhere(positive);
  values.where(positive);
  values.first;
  values.where(positive).isEmpty;
  values.where(positive).length == 0;
  objects.where((value) => value is String).first;
  objects.where((value) => (value is int)).last;
  objects.where((value) => value is num).single;
  nullable?.where(positive).first;
  unknown.where(positive).first;
  query.where(positive).first;
  query.where(positive).last;
  query.where(positive).single;
  custom.where(positive).first;
}

void membership(List<int> values) {
  values.indexOf(1) >= 0;
}

void streamSearch(Stream<int> values) {
  values.where(positive).first;
}
