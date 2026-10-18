typedef First = ({int zebra, int apple}); /* expect: record-fields-ordering */
typedef Mixed = (int, {int zebra, int apple}); /* expect: record-fields-ordering */
typedef Nested = ({({int zebra, int apple}) outer}); /* expect: record-fields-ordering */
({int zebra, int apple}) typed = (apple: 1, zebra: 2); /* expect: record-fields-ordering */
typedef Named = ({int zebra, int apple}); /* expect: record-fields-ordering */
