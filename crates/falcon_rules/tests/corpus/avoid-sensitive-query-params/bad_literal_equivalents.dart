final adjacentUrl = 'https://example.org?' 'token=secret'; /* expect: avoid-sensitive-query-params */
final adjacentQuery = Uri(query: 'to' 'ken=secret'); /* expect: avoid-sensitive-query-params */
final adjacentMapKey = Uri(queryParameters: {'to' 'ken': 'secret'}); /* expect: avoid-sensitive-query-params */
final escapedUrl = 'https://example.org?to\u006ben=secret'; /* expect: avoid-sensitive-query-params */
final escapedMap = Uri(queryParameters: {'to\x6ben': 'secret'}); /* expect: avoid-sensitive-query-params */
final rawAdjacent = r'https://example.org?' 'token=secret'; /* expect: avoid-sensitive-query-params */
