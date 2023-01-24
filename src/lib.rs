use leptos::{render_to_string, view};
use serde_json::json;
use view::app::{App, AppProps};
use worker::*;

mod utils;

trait Foo {
    fn some_method(&self);
}

struct Bar {
    thing: usize,
}

impl Foo for Bar {
    fn some_method(&self) {
        println!("self {}", self.thing);
    }
}

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get_async("/", |_, _| async move {
            let html = render_to_string(|cx| {
                return view! {cx,
                    <App />
                };
            });

            let mut headers = Headers::new();
            headers
                .append("Authorization", "Basic cHNhcm5hOjY5Ukl5MFo3SjVBQzhoMjQ=")
                .ok();
            let stmt = "INSERT INTO counter VALUES (4, 1000);";
            let request_init = RequestInit {
                body: Some(wasm_bindgen::JsValue::from_str(&format!(
                    "{{\"statements\": [\"{}\"]}}",
                    stmt
                ))),
                headers,
                cf: CfProperties::new(),
                method: Method::Post,
                redirect: RequestRedirect::Follow,
            };
            let mut req = Request::new_with_init(
                "http://iku-turso-809cf47c-9bce-11ed-801b-16cdfc4973c0-primary.fly.dev",
                &request_init,
            )
            .unwrap();
            let req_str = format!("{:?}", req);
            let response = Fetch::Request(req).send().await;
            let html = format!("Request: {}, Result: {:?}", req_str, response);

            return Response::from_html(html);
        })
        .post_async("/form/:field", |mut req, ctx| async move {
            if let Some(name) = ctx.param("field") {
                let form = req.form_data().await?;
                match form.get(name) {
                    Some(FormEntry::Field(value)) => {
                        return Response::from_json(&json!({ name: value }))
                    }
                    Some(FormEntry::File(_)) => {
                        return Response::error("`field` param in form shouldn't be a File", 422);
                    }
                    None => return Response::error("Bad Request", 400),
                }
            }

            Response::error("Bad Request", 400)
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
