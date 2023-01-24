use base64::Engine;
use worker::*;

pub struct Turso {
    url: String,
    auth: String,
}

impl Turso {
    pub fn connect(url: impl Into<String>, username: &str, pass: &str) -> Self {
        Self {
            url: url.into(),
            auth: format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, pass))
            ),
        }
    }

    pub async fn execute(&self, stmt: impl Into<String>) -> Result<String> {
        let mut headers = Headers::new();
        headers.append("Authorization", &self.auth).ok();
        let request_init = RequestInit {
            body: Some(wasm_bindgen::JsValue::from_str(&format!(
                "{{\"statements\": [\"{}\"]}}",
                stmt.into()
            ))),
            headers,
            cf: CfProperties::new(),
            method: Method::Post,
            redirect: RequestRedirect::Follow,
        };
        let req = Request::new_with_init(&self.url, &request_init)?;
        let response = Fetch::Request(req).send().await;
        Ok(response?.text().await?)
    }
}
