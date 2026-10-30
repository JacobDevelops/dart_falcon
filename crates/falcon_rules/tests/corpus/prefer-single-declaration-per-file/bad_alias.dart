class Base {}
mixin Marker {} /* expect: prefer-single-declaration-per-file */
class Alias = Base with Marker; /* expect: prefer-single-declaration-per-file */
