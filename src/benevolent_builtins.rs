//! The builtins whose declared failure branch is not worth enforcing.
//!
//! `tempnam()` returns `non-falsy-string|false`, `curl_init()` returns
//! `CurlHandle|false`, `Redis::get()` returns `string|false`. The failure
//! branch is real, but it fires only when something has gone wrong that the
//! caller could not act on locally anyway, so idiomatic PHP passes these
//! results straight on without checking. Enforcing the full union turns
//! ordinary filesystem, cache and date code into a stream of `|false`
//! argument mismatches.
//!
//! PHPStan models this by tagging the return types in its own function map
//! with `__benevolent<>`, and PHPantom borrows that list verbatim (both
//! projects are MIT licensed). The names below are every `__benevolent<>`
//! entry in PHPStan's `resources/functionMap.php`, lowercased because PHP
//! function and class names are case-insensitive, and sorted so the lookups
//! can binary-search them.
//!
//! Being on this list only *marks* the type (see
//! [`PhpType::benevolent`](crate::php_type::PhpType::benevolent)); the union
//! itself stays intact, so narrowing and `=== false` checks are unaffected.
//! Only the diagnostic compatibility check relaxes, and only for a type that
//! really is a union: an entry whose stub declares no failure branch on a
//! modern PHP (`substr()` and `round()` stopped returning `false` in PHP 8)
//! is tagged with nothing and passes through unchanged.

use std::cmp::Ordering;

/// Global functions with a benevolent return type.
static BENEVOLENT_FUNCTIONS: &[&str] = &[
    "apcu_cache_info",
    "apcu_sma_info",
    "ceil",
    "curl_init",
    "date_sun_info",
    "floor",
    "get_include_path",
    "getopt",
    "imagecreate",
    "imagecreatetruecolor",
    "mktime",
    "password_hash",
    "php_sapi_name",
    "round",
    "scandir",
    "stream_get_contents",
    "substr",
    "tempnam",
    "tmpfile",
];

/// `(class, method)` pairs with a benevolent return type.
static BENEVOLENT_METHODS: &[(&str, &str)] = &[
    ("closure", "bind"),
    ("closure", "bindto"),
    ("datetime", "modify"),
    ("datetimeimmutable", "modify"),
    ("domdocument", "createattribute"),
    ("domdocument", "createattributens"),
    ("domdocument", "createcdatasection"),
    ("domdocument", "createelement"),
    ("domdocument", "createelementns"),
    ("domdocument", "createentityreference"),
    ("domdocument", "createprocessinginstruction"),
    ("domelement", "getattributenode"),
    ("domelement", "getattributenodens"),
    ("domnode", "append_child"),
    ("domnode", "appendchild"),
    ("domnode", "c14n"),
    ("domnode", "c14nfile"),
    ("domnode", "clonenode"),
    ("domnode", "insertbefore"),
    ("domnode", "removechild"),
    ("domnode", "replacechild"),
    ("pdo", "prepare"),
    ("pdo", "quote"),
    ("redis", "append"),
    ("redis", "auth"),
    ("redis", "bgrewriteaof"),
    ("redis", "bgsave"),
    ("redis", "bitcount"),
    ("redis", "bitop"),
    ("redis", "bitpos"),
    ("redis", "blmove"),
    ("redis", "blmpop"),
    ("redis", "blpop"),
    ("redis", "brpop"),
    ("redis", "brpoplpush"),
    ("redis", "bzmpop"),
    ("redis", "bzpopmax"),
    ("redis", "bzpopmin"),
    ("redis", "copy"),
    ("redis", "dbsize"),
    ("redis", "debug"),
    ("redis", "decr"),
    ("redis", "decrby"),
    ("redis", "del"),
    ("redis", "delete"),
    ("redis", "discard"),
    ("redis", "dump"),
    ("redis", "echo"),
    ("redis", "exec"),
    ("redis", "exists"),
    ("redis", "expire"),
    ("redis", "expireat"),
    ("redis", "expiretime"),
    ("redis", "failover"),
    ("redis", "flushall"),
    ("redis", "flushdb"),
    ("redis", "function"),
    ("redis", "geoadd"),
    ("redis", "geodist"),
    ("redis", "geohash"),
    ("redis", "geopos"),
    ("redis", "georadius"),
    ("redis", "georadiusbymember"),
    ("redis", "georadiusbymember_ro"),
    ("redis", "geosearch"),
    ("redis", "geosearchstore"),
    ("redis", "getbit"),
    ("redis", "getdel"),
    ("redis", "getex"),
    ("redis", "getrange"),
    ("redis", "getset"),
    ("redis", "hdel"),
    ("redis", "hexists"),
    ("redis", "hget"),
    ("redis", "hgetall"),
    ("redis", "hincrby"),
    ("redis", "hincrbyfloat"),
    ("redis", "hkeys"),
    ("redis", "hlen"),
    ("redis", "hmget"),
    ("redis", "hmset"),
    ("redis", "hrandfield"),
    ("redis", "hscan"),
    ("redis", "hset"),
    ("redis", "hsetnx"),
    ("redis", "hstrlen"),
    ("redis", "hvals"),
    ("redis", "incr"),
    ("redis", "incrby"),
    ("redis", "incrbyfloat"),
    ("redis", "info"),
    ("redis", "keys"),
    ("redis", "lcs"),
    ("redis", "linsert"),
    ("redis", "llen"),
    ("redis", "lmove"),
    ("redis", "lmpop"),
    ("redis", "lpop"),
    ("redis", "lpos"),
    ("redis", "lpush"),
    ("redis", "lpushx"),
    ("redis", "lrange"),
    ("redis", "lrem"),
    ("redis", "lset"),
    ("redis", "ltrim"),
    ("redis", "mget"),
    ("redis", "migrate"),
    ("redis", "move"),
    ("redis", "mset"),
    ("redis", "msetnx"),
    ("redis", "multi"),
    ("redis", "object"),
    ("redis", "persist"),
    ("redis", "pexpireat"),
    ("redis", "pexpiretime"),
    ("redis", "pfadd"),
    ("redis", "pfcount"),
    ("redis", "pfmerge"),
    ("redis", "ping"),
    ("redis", "pipeline"),
    ("redis", "psetex"),
    ("redis", "pttl"),
    ("redis", "publish"),
    ("redis", "punsubscribe"),
    ("redis", "randomkey"),
    ("redis", "rename"),
    ("redis", "renamenx"),
    ("redis", "reset"),
    ("redis", "restore"),
    ("redis", "rpop"),
    ("redis", "rpoplpush"),
    ("redis", "rpush"),
    ("redis", "rpushx"),
    ("redis", "sadd"),
    ("redis", "save"),
    ("redis", "scard"),
    ("redis", "sdiff"),
    ("redis", "sdiffstore"),
    ("redis", "select"),
    ("redis", "set"),
    ("redis", "setbit"),
    ("redis", "setex"),
    ("redis", "setnx"),
    ("redis", "setrange"),
    ("redis", "sinter"),
    ("redis", "sintercard"),
    ("redis", "sinterstore"),
    ("redis", "sismember"),
    ("redis", "slaveof"),
    ("redis", "smembers"),
    ("redis", "smismember"),
    ("redis", "smove"),
    ("redis", "spop"),
    ("redis", "srandmember"),
    ("redis", "srem"),
    ("redis", "sscan"),
    ("redis", "strlen"),
    ("redis", "sunion"),
    ("redis", "sunionstore"),
    ("redis", "sunsubscribe"),
    ("redis", "swapdb"),
    ("redis", "time"),
    ("redis", "ttl"),
    ("redis", "type"),
    ("redis", "unlink"),
    ("redis", "unsubscribe"),
    ("redis", "unwatch"),
    ("redis", "watch"),
    ("redis", "xadd"),
    ("redis", "xclaim"),
    ("redis", "xdel"),
    ("redis", "xlen"),
    ("redis", "xpending"),
    ("redis", "xrange"),
    ("redis", "xread"),
    ("redis", "xreadgroup"),
    ("redis", "xrevrange"),
    ("redis", "xtrim"),
    ("redis", "zadd"),
    ("redis", "zcard"),
    ("redis", "zcount"),
    ("redis", "zincrby"),
    ("redis", "zinter"),
    ("redis", "zmpop"),
    ("redis", "zrange"),
    ("redis", "zrangebylex"),
    ("redis", "zrangebyscore"),
    ("redis", "zrank"),
    ("redis", "zrem"),
    ("redis", "zremrangebyrank"),
    ("redis", "zremrangebyscore"),
    ("redis", "zrevrange"),
    ("redis", "zrevrangebylex"),
    ("redis", "zrevrangebyscore"),
    ("redis", "zrevrank"),
    ("redis", "zscan"),
    ("redis", "zscore"),
    ("redis", "zunion"),
    ("rediscluster", "multi"),
    ("simplexmlelement", "addchild"),
    ("simplexmlelement", "attributes"),
    ("simplexmlelement", "children"),
    ("solrdocument", "getfield"),
    ("solrdocument", "getfieldcount"),
    ("solrdocument", "getfieldnames"),
    ("solrdocument", "getinputdocument"),
    ("splfileinfo", "getatime"),
    ("splfileinfo", "getgroup"),
    ("splfileinfo", "getinode"),
    ("splfileinfo", "getlinktarget"),
    ("splfileinfo", "getmtime"),
    ("splfileinfo", "getowner"),
    ("splfileinfo", "getpathinfo"),
    ("splfileinfo", "getperms"),
    ("splfileinfo", "getrealpath"),
    ("splfileinfo", "getsize"),
    ("splfileinfo", "gettype"),
];

/// Compare a lowercase table entry against a needle of any casing, without
/// allocating a lowercased copy of the needle.
fn cmp_lowercased(entry: &str, needle: &str) -> Ordering {
    let mut entry_bytes = entry.bytes();
    let mut needle_bytes = needle.bytes().map(|b| b.to_ascii_lowercase());
    loop {
        match (entry_bytes.next(), needle_bytes.next()) {
            (Some(a), Some(b)) if a == b => continue,
            (Some(a), Some(b)) => return a.cmp(&b),
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// Whether the global function `name` declares a failure branch nobody
/// checks.
pub fn function_is_benevolent(name: &str) -> bool {
    let name = name.strip_prefix('\\').unwrap_or(name);
    BENEVOLENT_FUNCTIONS
        .binary_search_by(|entry| cmp_lowercased(entry, name))
        .is_ok()
}

/// Whether `class::method` declares a failure branch nobody checks.
///
/// `class` is matched on its short name: every entry on the list is a
/// global-namespace builtin, and a project class that happens to share the
/// name will not be reaching this code, which runs only over stubs.
pub fn method_is_benevolent(class: &str, method: &str) -> bool {
    let class = crate::util::short_name(class);
    BENEVOLENT_METHODS
        .binary_search_by(|(entry_class, entry_method)| {
            cmp_lowercased(entry_class, class).then_with(|| cmp_lowercased(entry_method, method))
        })
        .is_ok()
}

/// Whether the class owns any benevolent method at all, so the common case
/// (every stub class that is not one of the dozen listed) costs one search
/// rather than one per method.
pub fn class_has_benevolent_methods(class: &str) -> bool {
    let class = crate::util::short_name(class);
    let start = BENEVOLENT_METHODS
        .partition_point(|(entry_class, _)| cmp_lowercased(entry_class, class) == Ordering::Less);
    BENEVOLENT_METHODS
        .get(start)
        .is_some_and(|(entry_class, _)| cmp_lowercased(entry_class, class) == Ordering::Equal)
}
