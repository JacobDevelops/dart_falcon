//! Security lint rules.

use crate::meta::RuleSource;

crate::declare_rules! {
    Security;
    avoid_hardcoded_credentials::AvoidHardcodedCredentials => ("avoid-hardcoded-credentials", file, None, false, RuleSource::Falcon);
    avoid_weak_cryptographic_algorithms::AvoidWeakCryptographicAlgorithms => ("avoid-weak-cryptographic-algorithms", resolved, None, true, RuleSource::Falcon);
    prefer_random_secure::PreferRandomSecure => ("prefer-random-secure", resolved, None, false, RuleSource::Falcon);
    avoid_sensitive_query_params::AvoidSensitiveQueryParams => ("avoid-sensitive-query-params", resolved, None, false, RuleSource::Falcon);
}
