void alpha(int cat, int cut) {} /* expect: avoid-similar-names, msg: "The name `cut` is too similar to `cat` (similarity: 0.67)." */

void birch(
  int cat,
  int cats, /* expect: avoid-similar-names, msg: "The name `cats` is too similar to `cat` (similarity: 0.75)." */
  int cuts, /* expect: avoid-similar-names, msg: "The name `cuts` is too similar to `cats` (similarity: 0.75)." */
) {}
