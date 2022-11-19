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

    /// Only do a sample of work.
    #[arg(long, default_value = "49277")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::debug!("Initialised logging");

    log::debug!("Loading dotenv file if present");
    dotenv::dotenv().ok();
    let args = Args::parse();

    log::info!("Logging into Twitter with OAuth");
    let oauth2_client = auth::load_client(args.port);
    let (url, state, verifier) = auth::login_start(&oauth2_client).await?;
    open::that(url.to_string())?;
    log::debug!("Waiting for callback...");
    let params = ikkizous_bot::oauth2_callback::catch_callback(args.port).await;
    assert_eq!(state.secret(), params.state.secret());
    let access_token = auth::login_end(&oauth2_client, params.code, verifier).await?;

    let mut bot = Bot::new(access_token);
    log::info!("Fetching image metadata");
    let progress = indicatif::ProgressBar::new_spinner();
    progress.set_style(
        indicatif::ProgressStyle::with_template("{spinner:.blue} {msg}")
            .expect("invalid progress template")
            // For more spinners check out the cli-spinners project:
            // https://github.com/sindresorhus/cli-spinners/blob/master/spinners.json
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
    progress.set_message("Fetching...");
    progress.enable_steady_tick(std::time::Duration::from_millis(120));
    let image_refs = bot
        .fetch_liked_image_refs(args.sample)
        .await?;
    progress.finish_and_clear();

    log::info!("Downloading {} images", image_refs.len());
    let client = reqwest::Client::new();
    let progress = indicatif::ProgressBar::new(image_refs.len().try_into().expect("usize in u64") );
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
