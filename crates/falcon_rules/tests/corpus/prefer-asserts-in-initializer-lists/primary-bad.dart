// @dart = 3.13

extension type SecondaryRepresentation(int? value) {
  SecondaryRepresentation.checked(this.value) {
    assert(value != null); /* expect: prefer-asserts-in-initializer-lists */
  }
}

extension type PrimaryRepresentation(int? value) {
  this {
    assert(value != null); /* expect: prefer-asserts-in-initializer-lists */
  }
}

class PrimaryParameter(int? value) {
  this {
    assert(value != null); /* expect: prefer-asserts-in-initializer-lists */
  }
}
