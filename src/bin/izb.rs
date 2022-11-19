use anyhow::Result;
use clap::Parser;
use ikkizous_bot::bot::Bot;
use ikkizous_bot::config::RedactedString;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Username of the likes to download
    #[arg(long)]
    username: String,

    /// Output directory to store files in.
    #[arg(long)]
    out_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    let twitter_bearer_token = RedactedString::new(
        std::env::var("TWITTER_BEARER_TOKEN").expect("TWITTER_BEARER_TOKEN unset"),
    );
    let bot = Bot::new(twitter_bearer_token);

    let image_refs = bot.fetch_liked_image_refs(&args.username).await?;

    let client = reqwest::Client::new();
    for image_ref in image_refs.into_iter() {
        let mut path = args.out_dir.clone();
        path.push(image_ref.filename);
        let mut file = File::create(&path)?;
        let bytes = &client.get(image_ref.url).send().await?.bytes().await?;
        file.write_all(bytes)?;
    }

    Ok(())
}
