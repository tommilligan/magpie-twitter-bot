use anyhow::{Context, Result};
use clap::Parser;
use magpie_twitter_bot::{auth, bot::Bot, download};
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
    let oauth2_client = auth::load_client(args.port).context("Loading OAuth2 configuration")?;
    let (url, state, verifier) = auth::login_start(&oauth2_client);

    open::that(url.to_string()).context("Failed to start login flow")?;
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    log::debug!("Waiting for callback...");
    let params = oneshot_oauth2_callback::oneshot(&address)
        .await
        .context("Login error")?;
    assert_eq!(state.secret(), params.state.secret());
    let access_token = auth::login_end(&oauth2_client, params.code, verifier)
        .await
        .context("Failed to fetch access token")?;

    let mut bot = Bot::new(access_token);
    log::info!("Fetching image metadata");
    let progress = arrow_spinner("Fetching...");
    let image_refs = bot
        .fetch_liked_image_refs(args.sample)
        .await
        .context("Failed to fetch image metadata")?;
    progress.finish_and_clear();

    log::info!("Downloading {} images", image_refs.len());
    std::fs::create_dir_all(&args.out_dir).with_context(|| {
        format!(
            "Failed to create output directory '{}'",
            args.out_dir.display()
        )
    })?;
    let client = reqwest::Client::new();
    let progress = indicatif::ProgressBar::new(image_refs.len().try_into().expect("usize in u64"));
    for image_ref in image_refs.into_iter() {
        let mut path = args.out_dir.clone();
        path.push(image_ref.filename());
        download::file(&client, image_ref.url.clone(), &path)
            .await
            .with_context(|| {
                format!("Failed writing '{}' to '{}'", image_ref.url, path.display())
            })?;
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
        log::error!("Runtime error:");
        for error in error.chain() {
            log::error!("--> {}", error);
        }
    }
}
