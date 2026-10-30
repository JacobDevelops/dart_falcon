void tied(
  int cater,
  int coter, /* expect: avoid-similar-names, msg: "The name `coter` is too similar to `cater`." */
  int cuter, /* expect: avoid-similar-names, msg: "The name `cuter` is too similar to `coter`." */
) {}

void betterEarlier(
  int customers,
  int custome,
  int customer, /* expect: avoid-similar-names, msg: "The name `customer` is too similar to `customers`." */
) {}

extension type Representation(int value) {
  int get values => value; /* expect: avoid-similar-names */
}
