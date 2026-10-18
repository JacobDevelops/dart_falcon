final value = (1, 2, 3); /* expect: avoid-long-records */
typedef LargeRecord = (int, int, {int third}); /* expect: avoid-long-records */
