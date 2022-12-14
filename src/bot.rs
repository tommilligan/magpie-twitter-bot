use std::collections::HashMap;
use thiserror::Error;
use time::format_description::well_known::Iso8601;
use tokio::sync::RwLock;
use twitter_v2::api_result::{ApiResponse, PaginableApiResponse};
use twitter_v2::authorization::Oauth2Token;
use twitter_v2::data::{MediaType, Tweet};
use twitter_v2::id::NumericId;
use twitter_v2::meta::{PaginationMeta, ResultCountMeta};
use twitter_v2::query::{MediaField, TweetExpansion, TweetField, UserField};
use twitter_v2::TwitterApi;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Twitter API violated an expected invariant: {}", 0)]
    TwitterApiInvariant(&'static str),
    #[error("Twitter client error")]
    TwitterClient(#[from] twitter_v2::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Helper to unwrap lots of optional fields from the twitter api, which are
/// guaranteed to be filled in response to certain requests.
trait TwitterInvariantExt<T> {
    fn ok_or_invariant(self, description: &'static str) -> Result<T>;
}

impl<T> TwitterInvariantExt<T> for Option<T> {
    fn ok_or_invariant(self, description: &'static str) -> Result<T> {
        self.ok_or(Error::TwitterApiInvariant(description))
    }
}

pub type UsernameCache = RwLock<HashMap<NumericId, String>>;

pub struct Bot {
    api: TwitterApi<Oauth2Token>,
    username_cache: UsernameCache,
}

#[derive(Debug, Clone)]
pub struct TweetRef {
    pub created_at: time::OffsetDateTime,
    pub username: String,
    pub id: NumericId,
}

#[derive(Debug, Clone)]
pub struct ImageRef {
    pub tweet: TweetRef,
    pub internal_filename: String,
    pub url: url::Url,
}

impl ImageRef {
    pub fn filename(&self) -> String {
        let created_at = self
            .tweet
            .created_at
            .format(&Iso8601::DEFAULT)
            .expect("format created at");
        format!(
            "{} {} {} {}",
            created_at, self.tweet.username, self.tweet.id, self.internal_filename
        )
    }
}

pub type Page = ApiResponse<Oauth2Token, Vec<Tweet>, ResultCountMeta>;

impl Bot {
    pub fn new(access_token: Oauth2Token) -> Self {
        let api = TwitterApi::new(access_token);
        Self {
            api,
            username_cache: Default::default(),
        }
    }

    /// Fetch liked tweets with associated metadata like image references.
    async fn fetch_liked_tweets_first(&self) -> Result<Page> {
        let user = self
            .api
            .get_users_me()
            .send()
            .await?
            .into_data()
            .ok_or_invariant("logged in user to exist")?;
        let first_page = self
            .api
            .get_user_liked_tweets(user.id)
            .tweet_fields([
                TweetField::Id,
                TweetField::Attachments,
                TweetField::Text,
                TweetField::AuthorId,
                TweetField::Entities,
                TweetField::CreatedAt,
            ])
            .expansions([TweetExpansion::AttachmentsMediaKeys])
            .media_fields([MediaField::Type, MediaField::Url])
            .send()
            .await?;
        Ok(first_page)
    }

    /// Fetch liked tweets with associated metadata like image references.
    pub fn fetch_liked_tweets<'a>(&'a self) -> impl futures::Stream<Item = Result<Page>> + 'a {
        enum State {
            Unintialised,
            Errored,
            Page(Page),
        }

        let stream = futures::stream::unfold(State::Unintialised, move |state| async move {
            let next_page: Result<Option<Page>> = match state {
                State::Unintialised => self.fetch_liked_tweets_first().await.map(|page| Some(page)),
                State::Page(current_page) => current_page.next_page().await.map_err(Into::into),
                State::Errored => return None,
            };
            let next_page: Option<Result<Page>> = next_page.transpose();
            next_page.map(|next_page| {
                let next_state: State = match next_page.as_ref() {
                    Ok(next_page) => State::Page(next_page.to_owned()),
                    Err(_) => State::Errored,
                };
                (next_page, next_state)
            })
        });
        stream
    }

    /// Process tweets with metadata into
    pub async fn process_page(&self, page: &Page) -> Result<Vec<ImageRef>> {
        let liked_tweets = match page.data() {
            Some(data) => data.to_owned(),
            // If not data, this is the last page and we will stop paginating.
            None => {
                assert_eq!(page.meta().and_then(|m| m.next_token()), None);
                return Ok(Vec::new());
            }
        };
        let includes = page
            .includes()
            .ok_or_invariant("includes in response")?
            .to_owned();

        let includes_media: HashMap<_, _> = includes
            .media
            .ok_or_invariant("media in includes")?
            .into_iter()
            .map(|media| (media.media_key.clone(), media))
            .collect();
        let mut image_refs = Vec::new();

        for tweet in liked_tweets.into_iter() {
            let author_id = tweet.author_id.ok_or_invariant("author id in tweet")?;
            let guard = self.username_cache.read().await;
            let username = guard.get(&author_id).cloned();
            drop(guard);
            let username = match username {
                Some(username) => username,
                None => {
                    let username = self
                        .api
                        .get_user(author_id)
                        .user_fields([UserField::Username])
                        .send()
                        .await?
                        .into_data()
                        .ok_or_invariant("username in response")?
                        .username;
                    let mut guard = self.username_cache.write().await;
                    guard.insert(author_id, username);
                    let username = guard
                        .get(&author_id)
                        .cloned()
                        .expect("just inserted author in cache");
                    drop(guard);
                    username
                }
            };

            let tweet_ref = TweetRef {
                username: username.to_owned(),
                created_at: tweet.created_at.ok_or_invariant("created_at in tweet")?,
                id: tweet.id,
            };

            if let Some(attachments) = tweet.attachments {
                for media_key in attachments
                    .media_keys
                    .ok_or_invariant("media_keys in attachments")?
                    .into_iter()
                {
                    if let Some(media) = includes_media.get(&media_key) {
                        if media.kind == MediaType::Photo {
                            let url = media.url.as_ref().ok_or_invariant("url in media")?;
                            let filename = url
                                .path_segments()
                                .ok_or_invariant("media url has valid path segments")?
                                .last()
                                .ok_or_invariant("media url has no path segments")?;
                            image_refs.push(ImageRef {
                                tweet: tweet_ref.clone(),
                                internal_filename: filename.to_owned(),
                                url: url.clone(),
                            })
                        }
                    }
                }
            }

            // Extract image from url in tweet
            // if let Some(entities) = tweet.entities {
            //     if let Some(urls) = entities.urls {
            //         for url in urls.into_iter() {
            //             if let Some(mut images) = url.images {
            //                 images.sort_by_key(|image| -(image.height as isize));
            //                 if let Some(image) = images.into_iter().next() {
            //                     let mut extension = std::borrow::Cow::Borrowed("jpg");
            //                     for (key, value) in image.url.query_pairs() {
            //                         if key == "format" {
            //                             extension = value;
            //                         }
            //                     }
            //                     image_refs.push(ImageRef {
            //                         tweet: tweet_ref.clone(),
            //                         internal_filename: format!("url-link.{extension}"),
            //                         url: image.url,
            //                     })
            //                 };
            //             }
            //         }
            //     }
            // };
        }

        Ok(image_refs)
    }
}
