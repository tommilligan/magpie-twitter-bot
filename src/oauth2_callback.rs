use axum::{extract::RawQuery, response::Html, routing::get, Router};
use std::borrow::Cow;
use oauth2::{
    basic::BasicErrorResponse, AuthorizationCode, CsrfToken
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Mutex;
use tokio::sync::oneshot::{channel, Sender};

struct State {
    pub shutdown: Option<Sender<()>>,
    pub params: Option<CodeGrantResult>,
}

static STATE: Mutex<State> = Mutex::new(State {
    shutdown: None,
    params: None,
});

struct Headings<'a> {
    title: &'a str,
    subheader: Cow<'a, str>,
}

impl Headings<'static> {
    pub fn new(title: &'static str, subheader: &'static str) -> Headings<'static> {
        Headings {title, subheader: Cow::Borrowed(subheader)}
    }
}

trait ToHeadings {
    fn to_headings(&self) -> Headings<'_>;
}

#[derive(Deserialize)]
pub struct CodeGrantResponse {
    pub code: AuthorizationCode,
    pub state: CsrfToken,
}

impl ToHeadings for CodeGrantResponse {
    fn to_headings(&self) -> Headings{
        Headings{
            title: "You are now logged in.",
            subheader: Cow::Borrowed("Please close the window."),
        }
    }
}

impl ToHeadings for BasicErrorResponse {
    fn to_headings(&self) -> Headings{
        let error = self.error().as_ref();
        let subheader = match (self.error_description(), self.error_uri()) {
            (None, None) => Cow::Borrowed(error),
            (Some(description), None) => Cow::Owned(format!("{error}: {description}")),
            (None, Some(uri)) => Cow::Owned(format!("{error} ({uri})")),
            (Some(description), Some(uri)) => Cow::Owned(format!("{error}: {description} ({uri})")),
        };
        Headings{
            title: "Login failed.",
            subheader,
        }
    }
}

/// Private implementation of `Result` so we can implement deserialize as an untagged enum.
///
/// Once we've deserialized, translate to `Result` and use that.
#[derive(Deserialize)]
#[serde(untagged)]
enum CodeGrantResultCustom {
    Ok(CodeGrantResponse),
    Err(BasicErrorResponse),
}

pub type CodeGrantResult = Result<CodeGrantResponse, BasicErrorResponse>;

impl Into<CodeGrantResult> for CodeGrantResultCustom {
    fn into(self) -> Result<CodeGrantResponse, BasicErrorResponse> {
        match self {
            CodeGrantResultCustom::Ok(response) => Ok(response),
            CodeGrantResultCustom::Err(response) => Err(response),
        }
    }
}

impl<T, E> ToHeadings for Result<T, E> where T: ToHeadings, E: ToHeadings {
    fn to_headings(&self) -> Headings{
        match self {
            Ok(response) => response.to_headings(),
            Err(response) => response.to_headings(),
        }
    }
}


// TODO pull this out to a generic catcher for any serde struct
pub async fn catch_callback(port: u16) -> CodeGrantResult {
    log::debug!("Setting initial state");
    let (tx, rx) = channel::<()>();
    {
        let mut state = STATE.lock().expect("could not lock mutex");
        *state = State {
            shutdown: Some(tx),
            params: None,
        };
    }

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/oauth2/callback", get(oauth2_callback));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    log::debug!("Listening for OAuth2 callback on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            rx.await.ok();
        })
        .await
        .unwrap();

    let mut state = STATE.lock().expect("could not lock mutex");
    state.params.take().expect("params set").into()
}

async fn root() -> &'static str {
    "waiting for callback"
}

async fn health() -> &'static str {
    "ok"
}

fn login_failed_headings() -> Headings<'static> {
    Headings::new("Login failed.", "Received invalid OAuth2 response.")
}

async fn oauth2_callback(RawQuery(query): RawQuery) -> Html<String> {
    let params = if let Some(query) = query {
        let params: CodeGrantResultCustom = serde_urlencoded::from_str(&query).unwrap();
        let params: CodeGrantResult = params.into();
        Some(params)
    } else {
        None
    };
    let headings = if let Some(ref params) = params {
        params.to_headings()
    } else {
        login_failed_headings()
    };

    let html = format!(r#"<html>
    <body>
        <div style="
            width: 100%;
            top: 50%;
            margin-top: 100px;
            text-align: center;
            font-family: sans-serif;
        ">
            <h1>{}</h1>
            <h2>{}</h2>
        </div>
    </body>
</html>"#, headings.title, headings.subheader);
    let mut state = STATE.lock().expect("could not lock mutex");
    state.params = params;
    log::debug!("shutting down");
    if let Some(shutdown) = state.shutdown.take() {
        shutdown.send(()).expect("failed to send shutdown");
    }
    Html(html)
}
