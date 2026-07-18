void good() {
  var a = items.whereType<String>();
  var b = list.where((e) => e.isValid);
  var c = things.where((e) => e is! String);
  var d = values.where((e) => e is int && e > 0);
  var e = data.where((a, b) => a is int);
  var f = items.map((e) => e is String);
}

// `Query` is not an Iterable and has no `whereType` member, so `.whereType<T>()`
// does not exist on it: `.where((e) => e is T)` here is SUPPRESSED because the
// receiver type is positively proven. This holds for a typed parameter, for
// `this`, and for a constructor-call receiver.
// (Requires the corpus harness to attach a TypeIndex for this rule; without one
// the receiver is Unknown and these lines would fire.)
class Query {
  Query where(bool Function(dynamic) test) => this;

  void run(Query q) {
    q.where((e) => e is String);
    this.where((e) => e is int);
    Query().where((e) => e is num);
  }
}
