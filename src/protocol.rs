use std::fmt;

/// 支持的缓存命令枚举
/// 
/// 遵循单一职责原则（SRP）：protocol 模块只负责协议解析，
/// 不涉及存储或网络逻辑。
#[derive(Debug, PartialEq)]
pub enum Command {
    /// SET key value [EX ttl]
    Set {
        key: String,
        value: String,
        /// 过期时间，单位秒
        ttl: Option<u64>,
    },
    /// GET key
    Get { key: String },
    /// DEL key
    Del { key: String },
    /// 返回统计信息
    Stats,
    /// 未知命令
    Unknown(String),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Set { key, value, ttl } => {
                if let Some(ttl) = ttl {
                    write!(f, "SET {} {} EX {}", key, value, ttl)
                } else {
                    write!(f, "SET {} {}", key, value)
                }
            }
            Command::Get { key } => write!(f, "GET {}", key),
            Command::Del { key } => write!(f, "DEL {}", key),
            Command::Stats => write!(f, "STATS"),
            Command::Unknown(cmd) => write!(f, "UNKNOWN {}", cmd),
        }
    }
}

/// 解析类 Redis 文本协议
/// 
/// 支持格式：
/// - SET key value [EX ttl]\r\n
/// - GET key\r\n
/// - DEL key\r\n
/// - STATS\r\n
/// 
/// 遵循开闭原则（OCP）：通过枚举扩展新命令，无需修改解析器核心逻辑。
pub fn parse(input: &str) -> Result<Command, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let cmd = parts[0].to_uppercase();

    match cmd.as_str() {
        "SET" => parse_set(&parts),
        "GET" => parse_get(&parts),
        "DEL" => parse_del(&parts),
        "STATS" => Ok(Command::Stats),
        _ => Ok(Command::Unknown(parts[0].to_string())),
    }
}

fn parse_set(parts: &[&str]) -> Result<Command, ParseError> {
    if parts.len() < 3 {
        return Err(ParseError::MissingArgument {
            cmd: "SET".to_string(),
            expected: "SET key value [EX ttl]".to_string(),
        });
    }

    let key = parts[1].to_string();
    let value = parts[2].to_string();
    let ttl = if parts.len() >= 5 && parts[3].to_uppercase() == "EX" {
        match parts[4].parse::<u64>() {
            Ok(secs) => Some(secs),
            Err(_) => return Err(ParseError::InvalidTtl(parts[4].to_string())),
        }
    } else {
        None
    };

    Ok(Command::Set { key, value, ttl })
}

fn parse_get(parts: &[&str]) -> Result<Command, ParseError> {
    if parts.len() < 2 {
        return Err(ParseError::MissingArgument {
            cmd: "GET".to_string(),
            expected: "GET key".to_string(),
        });
    }
    Ok(Command::Get {
        key: parts[1].to_string(),
    })
}

fn parse_del(parts: &[&str]) -> Result<Command, ParseError> {
    if parts.len() < 2 {
        return Err(ParseError::MissingArgument {
            cmd: "DEL".to_string(),
            expected: "DEL key".to_string(),
        });
    }
    Ok(Command::Del {
        key: parts[1].to_string(),
    })
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    EmptyCommand,
    MissingArgument { cmd: String, expected: String },
    InvalidTtl(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyCommand => write!(f, "empty command"),
            ParseError::MissingArgument { cmd, expected } => {
                write!(f, "{} requires: {}", cmd, expected)
            }
            ParseError::InvalidTtl(s) => write!(f, "invalid TTL: {}", s),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_set_without_ttl() {
        let cmd = parse("SET mykey myvalue").unwrap();
        assert_eq!(
            cmd,
            Command::Set {
                key: "mykey".to_string(),
                value: "myvalue".to_string(),
                ttl: None,
            }
        );
    }

    #[test]
    fn test_parse_set_with_ttl() {
        let cmd = parse("SET mykey myvalue EX 60").unwrap();
        assert_eq!(
            cmd,
            Command::Set {
                key: "mykey".to_string(),
                value: "myvalue".to_string(),
                ttl: Some(60),
            }
        );
    }

    #[test]
    fn test_parse_get() {
        let cmd = parse("GET mykey").unwrap();
        assert_eq!(cmd, Command::Get { key: "mykey".to_string() });
    }

    #[test]
    fn test_parse_del() {
        let cmd = parse("DEL mykey").unwrap();
        assert_eq!(cmd, Command::Del { key: "mykey".to_string() });
    }

    #[test]
    fn test_parse_stats() {
        let cmd = parse("STATS").unwrap();
        assert_eq!(cmd, Command::Stats);
    }

    #[test]
    fn test_parse_unknown() {
        let cmd = parse("UNKNOWNCMD").unwrap();
        assert_eq!(cmd, Command::Unknown("UNKNOWNCMD".to_string()));
    }

    #[test]
    fn test_parse_empty() {
        let err = parse("").unwrap_err();
        assert_eq!(err, ParseError::EmptyCommand);
    }

    #[test]
    fn test_parse_set_missing_args() {
        let err = parse("SET mykey").unwrap_err();
        assert_eq!(
            err,
            ParseError::MissingArgument {
                cmd: "SET".to_string(),
                expected: "SET key value [EX ttl]".to_string(),
            }
        );
    }
}
