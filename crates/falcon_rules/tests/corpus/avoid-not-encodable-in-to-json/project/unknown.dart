class MissingParent extends MissingBase {}
class MissingContract implements MissingInterface {}
class UnknownSerialized {
  Object toJson(MissingParent parent, MissingContract contract) => {
    'parent': parent,
    'contract': contract,
  };
}
