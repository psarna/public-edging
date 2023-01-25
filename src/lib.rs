use leptos::{render_to_string, view, IntoView};
use libsql_client::{CellValue, ResultSet};
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
        .get_async("/", |req, ctx| async move {
            let db = libsql_client::Session::connect_from_ctx(&ctx)?;
            // Note: this counter update code is subject to races, because it reads the value
            // first, and then updates it, and the operations are not atomic. It's only done
            // like that for demonstration purposes, please refrain from complaining online
            // that the code is not correct!

            db.transaction([
                "CREATE TABLE IF NOT EXISTS counter(key int PRIMARY KEY, value)",
                "INSERT INTO counter VALUES ('turso', 0)",
            ])
            .await
            .ok();

            db.execute(format!(
                "INSERT INTO counter VALUES ('{}', 'region was used!')",
                req.cf().region().unwrap_or_else(|| "unknown region".into())
            ))
            .await
            .ok();

            let response = db
                .execute("SELECT * FROM counter WHERE key = 'turso'")
                .await?;
            let counter_value = match response {
                ResultSet::Error((msg, _)) => {
                    return Response::from_html(format!("Error: {}", msg))
                }
                ResultSet::Success((result, _)) => {
                    let first_row = result
                        .rows
                        .first()
                        .ok_or(worker::Error::from("No rows found in the counter table"))?;
                    match first_row.cells.get("value") {
                        Some(Some(v)) => match v {
                            CellValue::Number(v) => *v,
                            _ => return Response::from_html("Unexpected counter value"),
                        },
                        _ => return Response::from_html("No value for 'value' column"),
                    }
                }
            };

            db.transaction([format!(
                "UPDATE counter SET value = {} WHERE key = 'turso'",
                counter_value + 1
            )])
            .await
            .ok();

            let counter_status = format!(
                "Counter was just successfully bumped: {} -> {}. Congrats!",
                counter_value,
                counter_value + 1,
            );
            // Author note: forgive me for all the inlined HTML, I don't speak leptos
            let mut html =
                "And here's the whole database, dumped: <br /><table style=\"border: 1px solid\">"
                    .to_string();
            let response = db.execute("SELECT * FROM counter").await?;
            match response {
                ResultSet::Error((msg, _)) => {
                    return Response::from_html(format!("Error: {}", msg))
                }
                ResultSet::Success((result, _)) => {
                    for column in &result.columns {
                        html += &format!("<th style=\"border: 1px solid\">{}</th>", column);
                    }
                    for row in result.rows {
                        html += "<tr style=\"border: 1px solid\">";
                        for column in &result.columns {
                            html += &format!("<td>{:?}</td>", row.cells[column]);
                        }
                        html += "</tr>";
                    }
                }
            };
            html += "</table>";

            let html = render_to_string(|cx| {
                view! {cx,
                    {counter_status} <br /><br />
                    {html} <br />
                    <App />
                }
            });
            Response::from_html(html)
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
