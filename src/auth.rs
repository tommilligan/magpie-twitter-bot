use twitter_v2::authorization::{Oauth2Client, Oauth2Token, Scope};
use twitter_v2::oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier};
use twitter_v2::Result;

pub fn load_client(port: u16) -> Oauth2Client {
    Oauth2Client::new(
        std::env::var("TWITTER_OAUTH_CLIENT_ID").expect("could not find CLIENT_ID"),
        std::env::var("TWITTER_OAUTH_CLIENT_SECRET").expect("could not find CLIENT_SECRET"),
        format!("http://localhost:{port}/oauth2/callback")
            .parse()
            .expect("callback url invalid"),
    )
}

pub fn login_start(client: &Oauth2Client) -> (url::Url, CsrfToken, PkceCodeVerifier) {
    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    // create authorization url
    let (url, state) = client.auth_url(
        challenge,
        [Scope::TweetRead, Scope::UsersRead, Scope::LikeRead],
    );
    // redirect user
    (url, state, verifier)
}

pub async fn login_end(
    client: &Oauth2Client,
    code: AuthorizationCode,
    verifier: PkceCodeVerifier,
) -> Result<Oauth2Token> {
    // request oauth2 token
    let token = client.request_token(code, verifier).await?;
    Ok(token)
}
