class BranchyOperator {
  @override
  bool /* operator in a comment */ operator ==(Object other) { /* expect: cyclomatic-complexity */
    return other != null && other is BranchyOperator && hashCode > 0 && hashCode < 100;
  }
}

List<int> collectionCondition(bool a, bool b, bool c, bool d) => [ /* expect: cyclomatic-complexity */
  if (a && b && c && d) 1,
];

String interpolated(bool a, bool b, bool c, bool d) => '${a && b && c && d}'; /* expect: cyclomatic-complexity */

int guardedSwitch(int value, bool a, bool b, bool c) => switch (value) { /* expect: cyclomatic-complexity */
  _ when a && b && c => 1,
};

void patternInitializer(bool a, bool b, bool c, bool d) { /* expect: cyclomatic-complexity */
  final (value, other) = (a && b && c && d, 0);
}

void nestedDecisions(bool a, bool b, bool c) { /* expect: cyclomatic-complexity */
  void local() {
    if (a) {}
  }
  final callbacks = [() {
    if (b) {}
    if (c) {}
  }];
}

void guardedStatement(int value, bool a, bool b, bool c) { /* expect: cyclomatic-complexity */
  switch (value) {
    case _ when a && b && c:
      break;
  }
}
