void conditions(bool ready, int count, int other, String name) {
  if (ready && !ready) {} /* expect: avoid-contradictory-expressions */
  if (!ready || ready) {} /* expect: avoid-contradictory-expressions */
  if ((ready) && !(ready)) {} /* expect: avoid-contradictory-expressions */
  if (count == 2 && count != 2) {} /* expect: avoid-contradictory-expressions */
  if (count == other && other != count) {} /* expect: avoid-contradictory-expressions */
  if (name == 'Ada' || 'Ada' != name) {} /* expect: avoid-contradictory-expressions */
  if (count == 3 && count == 4) {} /* expect: avoid-contradictory-expressions */
  if (count < 4 && count > 4) {} /* expect: avoid-contradictory-expressions */
  if (count >= 4 && count < 4) {} /* expect: avoid-contradictory-expressions */
  if (count > 3 && count < 4) {} /* expect: avoid-contradictory-expressions */
  if (count <= -2 && count > -1) {} /* expect: avoid-contradictory-expressions */
  if (4 < count && count <= 4) {} /* expect: avoid-contradictory-expressions */
  if (count <= 4 || count >= 5) {} /* expect: avoid-contradictory-expressions */
  if (count != 1 || count != 2) {} /* expect: avoid-contradictory-expressions */
  if (count == 4 && count > 4) {} /* expect: avoid-contradictory-expressions */
  if (count >= 9007199254740993 && count < 9007199254740993) {} /* expect: avoid-contradictory-expressions */
}
