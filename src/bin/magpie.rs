use anyhow::{anyhow, Result};
use clap::Parser;
use magpie_twitter_bot::auth;
use magpie_twitter_bot::bot::Bot;
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

    /// Only do a sample of work.
    #[arg(long, default_value = "49277")]
    port: u16,
}

fn arrow_spinner(message: &'static str) -> indicatif::ProgressBar {
    let progress = indicatif::ProgressBar::new_spinner();
    progress.set_style(
        indicatif::ProgressStyle::with_template("{spinner:.blue} {msg}")
            .expect("invalid progress template")
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ]),
    );
    progress.set_message(message);
    progress.enable_steady_tick(std::time::Duration::from_millis(120));
    progress
}

async fn run(args: &Args) -> Result<()> {
    log::info!("Logging into Twitter with OAuth");
    let oauth2_client = auth::load_client(args.port);
    let (url, state, verifier) = auth::login_start(&oauth2_client).await?;
    open::that(url.to_string())?;
    log::debug!("Waiting for callback...");
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    let params = oneshot_oauth2_callback::oneshot(&address)
        .await
        .map_err(|error| anyhow!("OAuth2 callback received an error response: {}", error))?;
    assert_eq!(state.secret(), params.state.secret());
    let access_token = auth::login_end(&oauth2_client, params.code, verifier).await?;

    let mut bot = Bot::new(access_token);
    log::info!("Fetching image metadata");
    let progress = arrow_spinner("Fetching...");
    let image_refs = bot.fetch_liked_image_refs(args.sample).await?;
    progress.finish_and_clear();

    log::info!("Downloading {} images", image_refs.len());
    std::fs::create_dir_all(&args.out_dir)?;
    let client = reqwest::Client::new();
    let progress = indicatif::ProgressBar::new(image_refs.len().try_into().expect("usize in u64"));
    for image_ref in image_refs.into_iter() {
        let mut path = args.out_dir.clone();
        path.push(image_ref.filename());
        let mut file = File::create(&path)?;
        let bytes = &client.get(image_ref.url).send().await?.bytes().await?;
        file.write_all(bytes)?;
        progress.inc(1);
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::debug!("Initialised logging");
    let args = Args::parse();

    if let Err(error) = run(&args).await {
        log::error!("Runtime error: {}", error)
    }
}
