// Bad: `Object` field types and member return types should be more specific.
class Store {
  Object cache = {}; /* expect: no-object-declaration */

  Object build() => cache; /* expect: no-object-declaration */

  Object get value => cache; /* expect: no-object-declaration */

  Object operator +(int other) => cache; /* expect: no-object-declaration */
}

mixin Cacheable {
  Object? snapshot; /* expect: no-object-declaration */
}
