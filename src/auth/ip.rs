use ipnet::IpNet;
use std::net::IpAddr;
use std::str::FromStr;

/// CIDR IP 白名单解析与匹配器
#[derive(Debug, Clone)]
pub struct IpWhitelistMatcher {
    networks: Vec<IpNet>,
}

impl IpWhitelistMatcher {
    /// 从逗号分隔的 CIDR 字符串解析 (如 "192.168.1.0/24, 10.0.0.0/8")
    pub fn parse(cidr_str: &str) -> Result<Self, String> {
        let mut networks = Vec::new();
        for item in cidr_str.split(',') {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }

            // 支持单个 IP 地址（自动转换为 /32 或 /128）以及 CIDR 格式
            let net = if trimmed.contains('/') {
                IpNet::from_str(trimmed)
            } else if let Ok(ip) = IpAddr::from_str(trimmed) {
                IpNet::from_str(&format!("{}/{}", ip, if ip.is_ipv4() { 32 } else { 128 }))
            } else {
                return Err(format!("Invalid IP or CIDR address: {}", trimmed));
            };

            match net {
                Ok(n) => networks.push(n),
                Err(err) => return Err(format!("Failed to parse CIDR {}: {}", trimmed, err)),
            }
        }

        Ok(Self { networks })
    }

    /// 校验给定的 IP 字符串是否在许可网段内
    pub fn is_allowed(&self, ip_str: &str) -> bool {
        if self.networks.is_empty() {
            return true;
        }

        let ip = match IpAddr::from_str(ip_str.trim()) {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        self.networks.iter().any(|net| net.contains(&ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cidr_matching() {
        let matcher = IpWhitelistMatcher::parse("192.168.1.0/24, 10.0.0.1").unwrap();
        assert!(matcher.is_allowed("192.168.1.100"));
        assert!(matcher.is_allowed("10.0.0.1"));
        assert!(!matcher.is_allowed("10.0.0.2"));
        assert!(!matcher.is_allowed("172.16.0.1"));
        assert!(!matcher.is_allowed("invalid-ip"));
    }
}
