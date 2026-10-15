enum OldCtor {
  current.old(),
  @deprecated
  obsolete.old();

  @deprecated
  const OldCtor.old();
}

enum OldParameter {
  current.named(oldParameter: 0);

  const OldParameter.named({@deprecated int? oldParameter});
}
