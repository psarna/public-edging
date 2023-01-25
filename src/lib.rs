use libsql_client::{CellValue, ResultSet};
use serde_json::json;
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
            let db = libsql_client::Session::connect(
                "http://iku-turso-809cf47c-9bce-11ed-801b-16cdfc4973c0-primary.fly.dev",
                "psarna",
                "69RIy0Z7J5AC8h24",
            );
            // Note: this counter update code is subject to races, because it reads the value
            // first, and then updates it, and the operations are not atomic. It's only done
            // like that for demonstration purposes, please refrain from complaining online
            // that the code is not correct!
            let response = db
                .execute("SELECT * FROM counter WHERE key = 'turso'")
                .await?;
            let counter_value = match response {
                ResultSet::Error((msg, _)) => {
                    return Response::from_html(format!("Error: {}", msg))
                }
                ResultSet::Success((rows, _)) => {
                    let first_row = rows
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

            let mut html = format!(
                "Counter was just successfully bumped: {} -> {}. Congrats!",
                counter_value,
                counter_value + 1,
            );
            html += "<br><br> And here's the whole database, dumped: <br>";
            let response = db.execute("SELECT * FROM counter").await?;
            match response {
                ResultSet::Error((msg, _)) => {
                    return Response::from_html(format!("Error: {}", msg))
                }
                ResultSet::Success((rows, _)) => {
                    for row in rows.rows {
                        html += &format!("{:?} <br>", row);
                    }
                }
            };

            db.transaction([format!(
                "UPDATE counter SET value = {} WHERE key = 'turso'",
                counter_value + 1
            )])
            .await
            .ok();

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
