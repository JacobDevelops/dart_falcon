// Distance 11 over 20 characters is exactly the configured inclusive 0.45.
void oak(int aaaaaaaaabbbbbbbbbbb, int aaaaaaaaaccccccccccc) {} /* expect: avoid-similar-names */

// The cheap length bound must also retain an exact 0.45 candidate.
void pine(int aaaaaaaaa, int aaaaaaaaabbbbbbbbbbb) {} /* expect: avoid-similar-names */
