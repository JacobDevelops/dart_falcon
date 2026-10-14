enum OldCtor {
  current.old(), /* expect: deprecated-member-use-from-same-package */
  @deprecated
  obsolete.old();

  @deprecated
  const OldCtor.old();
}

enum OldParameter {
  current.named(oldParameter: 0); /* expect: deprecated-member-use-from-same-package */

  const OldParameter.named({@deprecated int? oldParameter});
}
