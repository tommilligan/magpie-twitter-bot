use anyhow::Result;
use clap::Parser;
use ikkizous_bot::auth;
use ikkizous_bot::bot::Bot;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory to store files in.
    #[arg(long)]
    out_dir: PathBuf,

    /// Only do a sample of work.
    #[arg(long, default_value = "false")]
    sample: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::debug!("Initialised logging");

    log::debug!("Loading dotenv file if present");
    dotenv::dotenv().ok();
    let args = Args::parse();

    log::info!("Logging into Twitter with OAuth");
    let oauth2_client = auth::load_client();
    let (url, state, verifier) = auth::login_start(&oauth2_client).await?;
    open::that(url.to_string())?;
    log::debug!("Waiting for callback...");
    let params = ikkizous_bot::oauth2_callback::catch_callback().await;
    assert_eq!(state.secret(), params.state.secret());
    let access_token = auth::login_end(&oauth2_client, params.code, verifier).await?;

    let mut bot = Bot::new(access_token);
    let image_refs = bot
        .fetch_liked_image_refs(args.sample)
        .await?;
    let image_refs_len = image_refs.len();

    log::info!("Downloading {image_refs_len} images");
    let client = reqwest::Client::new();
    // FIXME can used a fixed length download bar here
    for (image_index, image_ref) in image_refs.into_iter().enumerate() {
        log::debug!("Downloading image ({}/{})", image_index + 1, image_refs_len);
        let mut path = args.out_dir.clone();
        path.push(image_ref.filename());
        let mut file = File::create(&path)?;
        let bytes = &client.get(image_ref.url).send().await?.bytes().await?;
        file.write_all(bytes)?;
    }

    Ok(())
}
