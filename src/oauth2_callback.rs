use axum::{extract::Query, routing::get, Router, response::Html};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Mutex;
use tokio::sync::oneshot::{channel, Sender};
use oauth2::{AuthorizationCode, CsrfToken};

struct State {
    pub shutdown: Option<Sender<()>>,
    pub params: Option<CallbackParams>,
}

static STATE: Mutex<State> = Mutex::new(State {
    shutdown: None,
    params: None,
});

// TODO handle errors like http://localhost:49277/oauth2/callback?error=access_denied&state=9B2gPFsDYTUWSE_IsFvrbw
#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: AuthorizationCode,
    pub state: CsrfToken,
}

// TODO pull this out to a generic catcher for any serde struct
pub async fn catch_callback(port: u16) -> CallbackParams {
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
    state.params.take().expect("params set")
}

async fn root() -> &'static str {
    "waiting for callback"
}

async fn health() -> &'static str {
    "ok"
}

async fn oauth2_callback(Query(callback_params): Query<CallbackParams>) -> Html<&'static str> {
    let mut state = STATE.lock().expect("could not lock mutex");
    state.params = Some(callback_params);
    log::debug!("shutting down");
    if let Some(shutdown) = state.shutdown.take() {
        shutdown.send(()).expect("failed to send shutdown");
    }
    Html(r#"<html>
    <body>
        <div style="
            width: 100%;
            top: 50%;
            margin-top: 100px;
            text-align: center;
            font-family: sans-serif;
        ">
            <h1>You are now logged in.</h1>
            <h2>Please close the window.</h2>
        </div>
    </body>
</html>"#)
}
