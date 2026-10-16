mixin EqualityOnly {
  @override
  bool operator ==(Object other) => true;
}

class MissingHash {
  @override
  bool operator ==(Object other) => true; /* expect: hash-and-equals */
}

mixin class MissingHashMixinClass {
  @override
  bool operator ==(Object other) => true; /* expect: hash-and-equals */
}
