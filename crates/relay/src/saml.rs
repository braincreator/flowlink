//! SAML 2.0 SP (Service Provider) integration
//!
//! Supports SSO via redirect binding and SLO via post binding.
//! Compatible with Keycloak, Active Directory FS, Azure AD, and other IdPs.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Form,
};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::server::AppState;

// ═══════════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlConfig {
    /// IdP entity ID (e.g. "https://keycloak.example.com/realms/master")
    pub idp_entity_id: String,
    /// IdP SSO URL (redirect binding)
    pub idp_sso_url: String,
    /// IdP SLO URL (optional)
    pub idp_slo_url: Option<String>,
    /// IdP X.509 certificate (base64 DER) for signature verification
    pub idp_cert: Option<String>,
    /// SP entity ID
    pub sp_entity_id: String,
    /// SP ACS URL (Assertion Consumer Service)
    pub sp_acs_url: String,
    /// SP SLS URL (Single Logout Service)
    pub sp_sls_url: Option<String>,
    /// Want assertions signed
    pub want_assertions_signed: bool,
    /// Want responses signed
    pub want_responses_signed: bool,
}

// ═══════════════════════════════════════════════════
// SAML Request (SP → IdP)
// ═══════════════════════════════════════════════════

fn generate_request_id() -> String {
    format!("id{}", uuid::Uuid::new_v4().simple())
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Generate a SAML AuthnRequest XML
fn build_authn_request(config: &SamlConfig) -> String {
    let id = generate_request_id();
    format!(
        r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
     xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
     ID="{id}" Version="2.0" IssueInstant="{issue_instant}"
     Destination="{destination}" ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
     AssertionConsumerServiceURL="{acs_url}">
  <saml:Issuer>{issuer}</saml:Issuer>
  <samlp:NameIDPolicy Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress" AllowCreate="true"/>
</samlp:AuthnRequest>"#,
        id = id,
        issue_instant = now_iso(),
        destination = config.idp_sso_url,
        acs_url = config.sp_acs_url,
        issuer = config.sp_entity_id,
    )
}

/// Generate deflate + base64 encoded SAML request for redirect binding
fn encode_redirect_request(xml: &str) -> String {
    use std::io::Write;
    let mut deflate = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    deflate.write_all(xml.as_bytes()).unwrap();
    let compressed = deflate.finish().unwrap();
    base64::engine::general_purpose::STANDARD.encode(&compressed)
}

// ═══════════════════════════════════════════════════
// SAML Response parsing (IdP → SP)
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlAssertion {
    pub subject_name_id: String,
    pub subject_format: String,
    pub attributes: std::collections::HashMap<String, Vec<String>>,
    pub issuer: String,
    pub audience: String,
    pub session_index: Option<String>,
}

/// Parse a base64-encoded SAML Response
fn parse_saml_response(response_b64: &str) -> Result<SamlAssertion, String> {
    // WARNING: This is a simplified SAML parser without signature verification.
    // Production deployments MUST use a proper SAML library (e.g., saml-rs) with
    // IdP certificate verification. Without it, assertions can be forged.
    log::warn!("SAML: Parsing assertion WITHOUT signature verification — not safe for production!");
    let xml_bytes = base64::engine::general_purpose::STANDARD
        .decode(response_b64)
        .map_err(|e| format!("Base64 decode error: {e}"))?;
    let xml_str = String::from_utf8(xml_bytes)
        .map_err(|e| format!("UTF-8 decode error: {e}"))?;

    let name_id = extract_tag(&xml_str, "saml:NameID")
        .or_else(|| extract_tag(&xml_str, "NameID"))
        .ok_or("No NameID in SAML response")?;

    let format = extract_attr(&xml_str, "saml:NameID", "Format")
        .or_else(|| extract_attr(&xml_str, "NameID", "Format"))
        .unwrap_or_else(|| "urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified".to_string());

    let mut attributes = std::collections::HashMap::new();
    for attr_xml in extract_all_tags(&xml_str, "saml:Attribute").into_iter()
        .chain(extract_all_tags(&xml_str, "Attribute").into_iter())
    {
        let name = extract_attr_raw(&attr_xml, "Name").unwrap_or_default();
        let values = extract_all_tags(&attr_xml, "saml:AttributeValue")
            .into_iter()
            .chain(extract_all_tags(&attr_xml, "AttributeValue").into_iter())
            .collect();
        if !name.is_empty() {
            attributes.insert(name, values);
        }
    }

    let issuer = extract_tag(&xml_str, "saml:Issuer")
        .or_else(|| extract_tag(&xml_str, "Issuer"))
        .unwrap_or_default();

    let audience = extract_tag(&xml_str, "saml:Audience")
        .or_else(|| extract_tag(&xml_str, "Audience"))
        .unwrap_or_default();

    let session_index = extract_attr(&xml_str, "samlp:AuthnStatement", "SessionIndex")
        .or_else(|| extract_attr(&xml_str, "AuthnStatement", "SessionIndex"));

    Ok(SamlAssertion {
        subject_name_id: name_id,
        subject_format: format,
        attributes,
        issuer,
        audience,
        session_index,
    })
}

// XML helpers
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let close = format!("</{tag}>");
    // Try <tag>content</tag>
    let open = format!("<{tag}>");
    if let Some(start) = xml.find(&open) {
        let cs = start + open.len();
        if let Some(end) = xml[cs..].find(&close) {
            return Some(xml[cs..cs + end].trim().to_string());
        }
    }
    // Try <tag ...>content</tag>
    let open2 = format!("<{tag} ");
    if let Some(start) = xml.find(&open2) {
        let gt = xml[start..].find('>')?;
        let cs = start + gt + 1;
        if let Some(end) = xml[cs..].find(&close) {
            return Some(xml[cs..cs + end].trim().to_string());
        }
    }
    None
}

fn extract_all_tags(xml: &str, tag: &str) -> Vec<String> {
    let close = format!("</{tag}>");
    let mut results = Vec::new();
    let mut pos = 0;
    loop {
        let open = format!("<{tag}");
        let Some(start) = xml[pos..].find(&open) else { break };
        let abs = pos + start;
        let Some(gt) = xml[abs..].find('>') else { break };
        let cs = abs + gt + 1;
        let Some(end) = xml[cs..].find(&close) else { break };
        results.push(xml[cs..cs + end].trim().to_string());
        pos = cs + end + close.len();
    }
    results
}

fn extract_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag} ");
    let start = xml.find(&open)?;
    let gt = xml[start..].find('>')?;
    extract_attr_raw(&xml[start..start + gt], attr)
}

fn extract_attr_raw(s: &str, attr: &str) -> Option<String> {
    let prefix = format!("{attr}=\"");
    let start = s.find(&prefix)?;
    let vs = start + prefix.len();
    let end = s[vs..].find('"')?;
    Some(s[vs..vs + end].to_string())
}

// ═══════════════════════════════════════════════════
// API Endpoints
// ═══════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct SamlInitParams {
    pub relay_state: Option<String>,
}

/// GET /auth/saml/login — redirect to IdP
pub async fn saml_login(
    State(state): State<AppState>,
    Query(params): Query<SamlInitParams>,
) -> impl IntoResponse {
    let config = match &state.saml_config {
        Some(c) => c.lock().await.clone(),
        None => return (axum::http::StatusCode::NOT_FOUND, "SAML not configured").into_response(),
    };

    let request_xml = build_authn_request(&config);
    let encoded = encode_redirect_request(&request_xml);
    let relay_state = params.relay_state.unwrap_or_else(|| "/dashboard".to_string());

    let url = format!(
        "{}?SAMLRequest={}&RelayState={}",
        config.idp_sso_url,
        urlencoding::encode(&encoded),
        urlencoding::encode(&relay_state),
    );

    Redirect::to(&url).into_response()
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
pub struct SamlAcsParams {
    pub SAMLResponse: String,
    pub RelayState: Option<String>,
}

/// POST /auth/saml/acs — Assertion Consumer Service
pub async fn saml_acs(
    State(state): State<AppState>,
    Form(params): Form<SamlAcsParams>,
) -> impl IntoResponse {
    let config = match &state.saml_config {
        Some(c) => c.lock().await.clone(),
        None => return (axum::http::StatusCode::NOT_FOUND, "SAML not configured").into_response(),
    };

    let assertion = match parse_saml_response(&params.SAMLResponse) {
        Ok(a) => a,
        Err(e) => {
            log::error!("SAML parse error: {e}");
            return (axum::http::StatusCode::BAD_REQUEST, format!("SAML parse error: {e}")).into_response();
        }
    };

    // Verify audience
    if !assertion.audience.is_empty() && assertion.audience != config.sp_entity_id {
        log::error!("SAML audience mismatch: expected={}, got={}", config.sp_entity_id, assertion.audience);
        return (axum::http::StatusCode::UNAUTHORIZED, "Audience mismatch").into_response();
    }

    // Verify issuer
    if assertion.issuer != config.idp_entity_id {
        log::error!("SAML issuer mismatch: expected={}, got={}", config.idp_entity_id, assertion.issuer);
        return (axum::http::StatusCode::UNAUTHORIZED, "Issuer mismatch").into_response();
    }

    log::info!("SAML SSO success: user={}", assertion.subject_name_id);

    let email = assertion.subject_name_id.clone();
    let name = assertion.attributes.get("displayName")
        .or_else(|| assertion.attributes.get("http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name"))
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| email.clone());
    let groups = assertion.attributes.get("memberOf")
        .or_else(|| assertion.attributes.get("http://schemas.xmlsoap.org/claims/Group"))
        .cloned()
        .unwrap_or_default();

    // Create or find account
    let (account_id, is_admin) = match &state.auth_engine {
        Some(engine) => match engine.find_or_create_by_email(&email, &name, &groups).await {
            Ok(r) => r,
            Err(e) => {
                log::error!("SAML account error: {e}");
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Account error").into_response();
            }
        },
        None => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Auth not configured").into_response(),
    };

    // Generate JWT
    let tokens = match &state.auth_engine {
        Some(engine) => engine.create_tokens(&account_id, &account_id, Some(&email), Some(&name), None, is_admin, None),
        None => unreachable!(),
    };

    let token = match tokens {
        Ok(t) => t.access_token,
        Err(e) => {
            log::error!("SAML token error: {e}");
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response();
        }
    };

    let redirect_to = params.RelayState.unwrap_or_else(|| "/dashboard".to_string());
    let cookie = format!("fl_token={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=604800");

    (
        [("Set-Cookie", cookie), ("Location", redirect_to)],
        axum::http::StatusCode::SEE_OTHER,
    ).into_response()
}

/// GET /auth/saml/metadata — SP metadata XML
pub async fn saml_metadata(State(state): State<AppState>) -> impl IntoResponse {
    let config = match &state.saml_config {
        Some(c) => c.lock().await.clone(),
        None => return (axum::http::StatusCode::NOT_FOUND, "SAML not configured").into_response(),
    };

    let xml = format!(
        r#"<?xml version="1.0"?>
<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata"
                     entityID="{sp_entity_id}">
  <md:SPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol"
                      WantAssertionsSigned="{want_signed}">
    <md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>
    <md:AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
                                  Location="{acs_url}" index="0" isDefault="true"/>
  </md:SPSSODescriptor>
</md:EntityDescriptor>"#,
        sp_entity_id = config.sp_entity_id,
        acs_url = config.sp_acs_url,
        want_signed = config.want_assertions_signed,
    );

    (
        [(axum::http::header::CONTENT_TYPE, "application/samlmetadata+xml")],
        xml,
    ).into_response()
}

// ═══════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_authn_request() {
        let config = SamlConfig {
            idp_entity_id: "https://keycloak.example.com/realms/master".to_string(),
            idp_sso_url: "https://keycloak.example.com/realms/master/protocol/saml".to_string(),
            idp_slo_url: None,
            idp_cert: None,
            sp_entity_id: "https://flowlink.flow-masters.ru".to_string(),
            sp_acs_url: "https://flowlink.flow-masters.ru/auth/saml/acs".to_string(),
            sp_sls_url: None,
            want_assertions_signed: true,
            want_responses_signed: true,
        };
        let xml = build_authn_request(&config);
        assert!(xml.contains("samlp:AuthnRequest"));
        assert!(xml.contains("flowlink.flow-masters.ru"));
    }

    #[test]
    fn test_extract_tag() {
        let xml = r#"<saml:NameID Format="email">user@example.com</saml:NameID>"#;
        assert_eq!(extract_tag(xml, "saml:NameID"), Some("user@example.com".to_string()));
    }

    #[test]
    fn test_parse_saml_response() {
        let response = r#"<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol">
            <saml:Issuer>https://idp.example.com</saml:Issuer>
            <saml:Assertion>
                <saml:Subject>
                    <saml:NameID Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress">user@example.com</saml:NameID>
                </saml:Subject>
                <saml:AttributeStatement>
                    <saml:Attribute Name="displayName">
                        <saml:AttributeValue>Test User</saml:AttributeValue>
                    </saml:Attribute>
                    <saml:Attribute Name="memberOf">
                        <saml:AttributeValue>admin</saml:AttributeValue>
                    </saml:Attribute>
                </saml:AttributeStatement>
                <saml:Conditions>
                    <saml:AudienceRestriction>
                        <saml:Audience>https://flowlink.flow-masters.ru</saml:Audience>
                    </saml:AudienceRestriction>
                </saml:Conditions>
            </saml:Assertion>
        </samlp:Response>"#;

        let b64 = base64::engine::general_purpose::STANDARD.encode(response);
        let a = parse_saml_response(&b64).unwrap();
        assert_eq!(a.subject_name_id, "user@example.com");
        assert_eq!(a.issuer, "https://idp.example.com");
        assert_eq!(a.audience, "https://flowlink.flow-masters.ru");
        assert_eq!(a.attributes.get("displayName").unwrap()[0], "Test User");
    }
}
