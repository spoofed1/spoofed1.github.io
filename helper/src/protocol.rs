//! Only the app's current, explicitly opened receive session authorizes delivery.
use serde::Deserialize;

pub const APP_ID: &str = "com.yashpattekar.myapp";
pub const REQUEST: &str = "Documents/spoofed-setup-request.json";
pub const PARTIAL: &str = "Documents/spoofed-pairing.partial";
pub const DELIVERY: &str = "Documents/spoofed-pairing.plist";
pub const RECEIPT: &str = "Documents/spoofed-setup-result.json";
pub const LIMIT: usize = 4096;

#[derive(Deserialize)]
struct Request {
    version: u32,
    nonce: String,
}

pub fn request_nonce(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > LIMIT {
        return Err("The setup request is too large. Restart receiving in Spoofed.".into());
    }
    let request: Request = serde_json::from_slice(bytes)
        .map_err(|_| "Open Spoofed → Setup → Receive from Computer, then try again.")?;
    if request.version != 1 || uuid::Uuid::parse_str(&request.nonce).is_err() {
        return Err("Update Spoofed and restart Receive from Computer.".into());
    }
    Ok(request.nonce)
}

pub fn envelope(nonce: &str, credentials: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut value = plist::Dictionary::new();
    value.insert("nonce".into(), nonce.into());
    value.insert("pairing".into(), plist::Value::Data(credentials));
    let mut bytes = Vec::new();
    plist::to_writer_xml(&mut bytes, &value).map_err(|_| "Couldn't prepare the transfer.")?;
    if bytes.len() > 1_500_000 {
        return Err("The pairing is too large to transfer.".into());
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct Receipt {
    nonce: String,
    accepted: bool,
}

pub fn receipt(bytes: &[u8], nonce: &str) -> Option<bool> {
    if bytes.len() > LIMIT {
        return None;
    }
    let result: Receipt = serde_json::from_slice(bytes).ok()?;
    (result.nonce == nonce).then_some(result.accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    const NONCE: &str = "2e21325f-e741-449c-8d7d-bb485bf2d600";
    #[test]
    fn rejects_incompatible_and_malformed_requests() {
        assert_eq!(
            request_nonce(format!(r#"{{"version":1,"nonce":"{NONCE}"}}"#).as_bytes()).unwrap(),
            NONCE
        );
        for bytes in [
            br#"{"version":2,"nonce":"x"}"#.as_slice(),
            b"{}",
            b"null",
            br#"{"version":1,"nonce":"bad"}"#,
        ] {
            assert!(request_nonce(bytes).is_err());
        }
        assert!(request_nonce(&vec![b' '; LIMIT + 1]).is_err());
        assert!(request_nonce(format!(r#"{{"version":2,"nonce":"{NONCE}"}}"#).as_bytes()).is_err());
    }
    #[test]
    fn confirmation_must_match_this_session() {
        assert_eq!(
            receipt(
                format!(r#"{{"nonce":"{NONCE}","accepted":true}}"#).as_bytes(),
                NONCE
            ),
            Some(true)
        );
        assert_eq!(
            receipt(
                format!(r#"{{"nonce":"{NONCE}","accepted":false}}"#).as_bytes(),
                NONCE
            ),
            Some(false)
        );
        assert_eq!(receipt(br#"{"nonce":"old","accepted":true}"#, NONCE), None);
        assert_eq!(receipt(b"{}", NONCE), None);
    }
    #[test]
    fn delivery_matches_the_iphone_contract() {
        let bytes = envelope(NONCE, vec![1, 2, 3]).unwrap();
        let value: plist::Value = plist::from_bytes(&bytes).unwrap();
        let dict = value.as_dictionary().unwrap();
        assert_eq!(dict["nonce"].as_string(), Some(NONCE));
        assert_eq!(dict["pairing"].as_data(), Some([1, 2, 3].as_slice()));
    }
}
