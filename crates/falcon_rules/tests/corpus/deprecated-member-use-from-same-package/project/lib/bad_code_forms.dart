class CodeForms {
  @deprecated
  int field = 0;
  @Deprecated('read old')
  int get value => 0;
  @Deprecated('write old')
  set value(int value) {}
  @deprecated
  void call() {}
  @deprecated
  CodeForms operator -() => this;
  CodeForms() : field = 1; /* expect: deprecated-member-use-from-same-package */
  void implicit() {
    field; /* expect: deprecated-member-use-from-same-package */
    field = 2; /* expect: deprecated-member-use-from-same-package */
    field += 1; /* expect: deprecated-member-use-from-same-package */
    ++field; /* expect: deprecated-member-use-from-same-package */
    value += 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  }
}
class SubForms extends CodeForms {
  void inherited() {
    super.field; /* expect: deprecated-member-use-from-same-package */
    super.field = 2; /* expect: deprecated-member-use-from-same-package */
    field; /* expect: deprecated-member-use-from-same-package */
  }
}
void positional([@deprecated int old = 0]) {}
class PositionalForms {
  PositionalForms([@deprecated int old = 0]);
  void method([@deprecated int old = 0]) {}
}
extension FormExtension on int {
  @deprecated
  void old() {}
  void implicit() {
    old(); /* expect: deprecated-member-use-from-same-package */
  }
}
@deprecated
extension OldFormExtension on String {
  void member() {}
}
@deprecated
class OldStaticForm {
  static int value = 0;
}
void useForms(CodeForms forms) {
  forms(); /* expect: deprecated-member-use-from-same-package */
  forms.call(); /* expect: deprecated-member-use-from-same-package */
  -forms; /* expect: deprecated-member-use-from-same-package */
  forms.value += 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  forms.field++; /* expect: deprecated-member-use-from-same-package */
  forms..field..field = 2..value += 1; /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
  positional(1); /* expect: deprecated-member-use-from-same-package */
  final parameters = PositionalForms(1); /* expect: deprecated-member-use-from-same-package */
  parameters.method(1); /* expect: deprecated-member-use-from-same-package */
  @deprecated
  void local() {}
  local(); /* expect: deprecated-member-use-from-same-package */
  void localParameters(int current, [@deprecated int old = 0]) {}
  localParameters(1, 2); /* expect: deprecated-member-use-from-same-package */
  void localNamed({@deprecated int old = 0}) {}
  localNamed(old: 2); /* expect: deprecated-member-use-from-same-package */
  1.old(); /* expect: deprecated-member-use-from-same-package */
  FormExtension(1).old(); /* expect: deprecated-member-use-from-same-package */
  OldFormExtension('').member(); /* expect: deprecated-member-use-from-same-package */
  OldStaticForm.value = 1; /* expect: deprecated-member-use-from-same-package */
  if (forms case CodeForms(field: final item)) {} /* expect: deprecated-member-use-from-same-package */
  if (forms case CodeForms(:final field)) {} /* expect: deprecated-member-use-from-same-package */
}

class StaticImplicitForms {
  @deprecated
  static int field = 0;
  void useStatic() {
    field; /* expect: deprecated-member-use-from-same-package */
    field = 1; /* expect: deprecated-member-use-from-same-package */
  }
}
