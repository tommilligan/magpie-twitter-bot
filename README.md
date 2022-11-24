<p align="center">
  <img src="./img/magpie-twitter-bot-icon.png" width="128px" height="128px">
  
  <h3 align="center">magpie-twitter-bot</h3>

  <p align="center">
    Hoard your valuable twitter likes with a bulk download.
  </p>
</p>

## About

Twitter offers a "Download my data" type facility, but it only downloads the bare bones of data
related to **you** - which accounts you follow, which tweets you liked, but without any of their associated data.

This bot crawls through your liked tweets, finding image attachments and downloading them for safekeeping.

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

You will need to enable user authentication settings as follows:

- App permissions: read
- Type of App: Confidential Client
- App info
  - Callback URI: `http://localhost:49277/oauth2/callback`
  - Website URL: `<your website url>`

## Run

Run the bot, giving it an output directory to store files in:

```bash
magpie --out-dir out
```
