use totp_rs::{Builder, Secret, Totp};

pub struct Code {
    pub code: String,
    pub remaining: u64,
    pub period: u64,
}

fn parse(secret: &str) -> Option<Totp> {
    let secret = secret.trim();
    if secret.starts_with("otpauth://") {
        return Totp::from_url_unchecked(secret).ok();
    }
    let clean: String = secret
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=' && *c != '-')
        .collect::<String>()
        .to_uppercase();
    let secret = Secret::try_from_base32(clean).ok()?;
    Some(Builder::new().with_secret(secret).build_noncompliant())
}

pub fn current(secret: &str) -> Option<Code> {
    let totp = parse(secret)?;
    Some(Code {
        code: totp.generate_current().to_string(),
        remaining: totp.ttl(),
        period: totp.next_step(0),
    })
}

pub fn pretty(code: &str) -> String {
    if code.len() == 6 {
        format!("{} {}", &code[..3], &code[3..])
    } else {
        code.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_vector() {
        let totp = parse("gezd gnbv gy3t qojq gezd gnbv gy3t qojq").unwrap();
        assert_eq!(totp.generate(59).to_string(), "287082");
        assert_eq!(totp.next_step(0), 30);
        let url = "otpauth://totp/x:me?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=x";
        assert_eq!(parse(url).unwrap().generate(59).to_string(), "287082");
        assert!(parse("not base32 !!").is_none());
        assert_eq!(pretty("287082"), "287 082");
    }
}
