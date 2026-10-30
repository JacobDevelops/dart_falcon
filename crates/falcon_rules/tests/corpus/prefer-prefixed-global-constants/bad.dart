const value = 1; /* expect: prefer-prefixed-global-constants, msg: "Prefix global constants with one of: `k`." */
const _private = 2; /* expect: prefer-prefixed-global-constants */
const int first = 1, second = 2; /* expect: prefer-prefixed-global-constants */ /* expect: prefer-prefixed-global-constants */
const another = 3; /* expect: prefer-prefixed-global-constants */
