<p align="center">
  <img src="./img/magpie-twitter-bot-icon.png">
  
  <h3 align="center">magpie-twitter-bot</h3>

  <p align="center">
    Hoard your valuable twitter likes with a bulk download.
  </p>
</p>

## Installation

Clone and install from this repository:

```bash
git clone https://github.com/tommilligan/magpie-twitter-bot
cd magpie-twitter-bot
cargo install --path .
```

## Credentials

You will need to provide the bot with a Twitter OAuth Client ID and secret.

For getting started with the Twitter API, check out [the official docs](https://developer.twitter.com/en/docs/twitter-api/getting-started/getting-access-to-the-twitter-api).

These credentials must be provided as environment variables:

```bash
TWITTER_OAUTH_CLIENT_ID=ABCDEF...
TWITTER_OAUTH_CLIENT_SECRET=GHIJKL...
```

## Run

Run the bot, giving it an output directory to store files in:

```bash
magpie --out-dir out
```
