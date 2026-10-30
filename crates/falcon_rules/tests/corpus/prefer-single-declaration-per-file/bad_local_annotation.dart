const visibleForTesting = 1;
class First {}
@visibleForTesting
class Testing {} /* expect: prefer-single-declaration-per-file */
