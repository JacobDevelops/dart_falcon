final conditional = Uri(queryParameters: {if (true) 'token': 'secret'}); /* expect: avoid-sensitive-query-params */
final alternative = Uri(queryParameters: {if (false) 'name': 'ok' else 'token': 'secret'}); /* expect: avoid-sensitive-query-params */
final spread = Uri(queryParameters: {...{'token': 'secret'}}); /* expect: avoid-sensitive-query-params */
final nested = Uri(queryParameters: {if (true) ...{for (var i in [1]) 'token': 'secret'}}); /* expect: avoid-sensitive-query-params */
