use crate::types::{FinancialQrPayload, MexicanSatCfdiPayload, SepaEpcPayload, SwissQrPayload};

/// Inspects and parses structured financial payloads from raw optical code strings.
pub fn parse_financial_payload(raw: &str) -> Option<FinancialQrPayload> {
    let trimmed = raw.trim();
    if trimmed.starts_with("BCD") {
        return parse_sepa_epc(trimmed).map(FinancialQrPayload::SepaEpc);
    }
    if trimmed.starts_with("SPC") {
        return parse_swiss_qr(trimmed).map(FinancialQrPayload::SwissQr);
    }
    if is_mexican_sat_candidate(trimmed) {
        return parse_mexican_sat(trimmed).map(FinancialQrPayload::MexicanSatCfdi);
    }
    None
}

/// Parses EPC SEPA QR codes (Quick Response Code Guidelines for SEPA Credit Transfers).
fn parse_sepa_epc(content: &str) -> Option<SepaEpcPayload> {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();
    if lines.len() < 7 {
        return None;
    }

    let service_tag = lines[0].to_string();
    let version = lines[1].to_string();
    let character_set = lines[2].to_string();
    let identification = lines[3].to_string();
    let bic = lines[4].to_string();
    let beneficiary_name = lines[5].to_string();
    let iban = lines[6].to_string();

    let (currency, amount) = parse_epc_amount(lines.get(7).copied());
    let purpose = parse_optional_line(lines.get(8).copied());
    let remittance_reference = parse_optional_line(lines.get(9).copied());
    let remittance_text = parse_optional_line(lines.get(10).copied());

    Some(SepaEpcPayload {
        service_tag,
        version,
        character_set,
        identification,
        bic,
        beneficiary_name,
        iban,
        amount,
        currency,
        purpose,
        remittance_reference,
        remittance_text,
    })
}

/// Helper to parse amount and currency in EPC SEPA QR codes.
fn parse_epc_amount(raw_opt: Option<&str>) -> (String, Option<f64>) {
    let raw = match raw_opt {
        Some(s) if !s.is_empty() => s,
        _ => return ("EUR".to_string(), None),
    };

    let (curr, num_str) = if let Some(stripped) = raw.strip_prefix("EUR") {
        ("EUR", stripped)
    } else if raw.len() > 3 && raw[..3].chars().all(|c| c.is_ascii_uppercase()) {
        (&raw[..3], &raw[3..])
    } else {
        ("EUR", raw)
    };

    let amount = num_str.trim().parse::<f64>().ok();
    (curr.to_string(), amount)
}

/// Parses Swiss QR-bill format (SPC specifications).
fn parse_swiss_qr(content: &str) -> Option<SwissQrPayload> {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();
    if lines.len() < 6 {
        return None;
    }

    let version = lines.get(1).unwrap_or(&"0200").to_string();
    let iban = lines.get(3).unwrap_or(&"").to_string();
    let creditor_name = lines.get(4).unwrap_or(&"").to_string();
    let creditor_address = lines.get(5).unwrap_or(&"").to_string();

    let amount = lines.get(18).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            s.parse::<f64>().ok()
        }
    });
    let currency = lines.get(19).unwrap_or(&"CHF").to_string();
    let reference = parse_optional_line(lines.get(28).copied());
    let message = parse_optional_line(lines.get(29).copied());

    Some(SwissQrPayload {
        version,
        currency: if currency.is_empty() {
            "CHF".to_string()
        } else {
            currency
        },
        amount,
        iban,
        creditor_name,
        creditor_address,
        reference,
        message,
    })
}

/// Checks whether a raw string looks like a Mexican SAT CFDI verification QR code.
fn is_mexican_sat_candidate(content: &str) -> bool {
    content.contains("sat.gob.mx")
        || (content.contains("re=") && content.contains("rr=") && content.contains("tt="))
}

/// Parses Mexican SAT CFDI electronic invoice verification URL parameters.
fn parse_mexican_sat(content: &str) -> Option<MexicanSatCfdiPayload> {
    let query_str = if let Some((_, query)) = content.split_once('?') {
        query
    } else {
        content
    };

    let mut uuid = None;
    let mut rfc_issuer = None;
    let mut rfc_receiver = None;
    let mut total = None;
    let mut fe = None;

    for param in query_str.split('&') {
        if let Some((key, val)) = param.split_once('=') {
            match key.trim() {
                "id" => uuid = Some(val.trim().to_string()),
                "re" => rfc_issuer = Some(val.trim().to_string()),
                "rr" => rfc_receiver = Some(val.trim().to_string()),
                "tt" => total = val.trim().parse::<f64>().ok(),
                "fe" => fe = Some(val.trim().to_string()),
                _ => {}
            }
        }
    }

    Some(MexicanSatCfdiPayload {
        uuid: uuid?,
        rfc_issuer: rfc_issuer?,
        rfc_receiver: rfc_receiver?,
        total: total.unwrap_or(0.0),
        fe: fe.unwrap_or_default(),
    })
}

/// Helper to convert empty string references into `None`.
fn parse_optional_line(line: Option<&str>) -> Option<String> {
    line.filter(|s| !s.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sepa_epc_success() {
        let epc = "BCD\n001\n1\nSCT\nTESTBIC\nAcme Corp\nES1234567890123456789012\nEUR450.75\n\nRF18539007547034\nInvoice 1234\n";
        let parsed = parse_financial_payload(epc).expect("should parse");
        if let FinancialQrPayload::SepaEpc(data) = parsed {
            assert_eq!(data.service_tag, "BCD");
            assert_eq!(data.beneficiary_name, "Acme Corp");
            assert_eq!(data.iban, "ES1234567890123456789012");
            assert_eq!(data.amount, Some(450.75));
            assert_eq!(data.currency, "EUR");
            assert_eq!(
                data.remittance_reference,
                Some("RF18539007547034".to_string())
            );
            assert_eq!(data.remittance_text, Some("Invoice 1234".to_string()));
        } else {
            panic!("Expected SepaEpc payload");
        }
    }

    #[test]
    fn test_parse_mexican_sat_cfdi_success() {
        let url = "https://verificacfdi.facturaelectronica.sat.gob.mx/default.aspx?id=12345678-ABCD-1234-ABCD-1234567890AB&re=AAA010101AAA&rr=BBB020202BBB&tt=1234.56&fe=abcdef12";
        let parsed = parse_financial_payload(url).expect("should parse");
        if let FinancialQrPayload::MexicanSatCfdi(data) = parsed {
            assert_eq!(data.uuid, "12345678-ABCD-1234-ABCD-1234567890AB");
            assert_eq!(data.rfc_issuer, "AAA010101AAA");
            assert_eq!(data.rfc_receiver, "BBB020202BBB");
            assert_eq!(data.total, 1234.56);
            assert_eq!(data.fe, "abcdef12");
        } else {
            panic!("Expected MexicanSatCfdi payload");
        }
    }
}
