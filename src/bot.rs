use std::collections::HashMap;
use time::format_description::well_known::Iso8601;
use twitter_v2::api_result::{ApiResponse, PaginableApiResponse};
use twitter_v2::authorization::Oauth2Token;
use twitter_v2::data::{MediaType, Tweet};
use twitter_v2::id::NumericId;
use twitter_v2::meta::{PaginationMeta, ResultCountMeta};
use twitter_v2::query::{MediaField, TweetExpansion, TweetField, UserField};
use twitter_v2::Result;
use twitter_v2::TwitterApi;

pub struct Bot {
    api: TwitterApi<Oauth2Token>,
    username_cache: HashMap<NumericId, String>,
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
            created_at,
            self.tweet.username,
            self.tweet.id.to_string(),
            self.internal_filename
        )
    }
}

impl Bot {
    pub fn new(access_token: Oauth2Token) -> Self {
        let api = TwitterApi::new(access_token);
        Self {
            api,
            username_cache: Default::default(),
        }
    }

    pub async fn fetch_liked_image_refs(
        &mut self,
        sample: bool,
    ) -> Result<Vec<ImageRef>> {
        let user = self
            .api
            .get_users_me()
            .send()
            .await?
            .into_data()
            .expect("username to exist");
        let mut next_page = Some(
            self.api
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
                .await?,
        );

        let mut image_refs = Vec::new();
        while let Some(page) = next_page {
            image_refs.extend(self.process_page(&page).await?);
            if sample {
                return Ok(image_refs);
            }

            next_page = page.next_page().await?;
        }

        Ok(image_refs)
    }

    pub async fn process_page(
        &mut self,
        page: &ApiResponse<Oauth2Token, Vec<Tweet>, ResultCountMeta>,
    ) -> Result<Vec<ImageRef>> {
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
            .expect("response contains includes")
            .to_owned();

        let includes_media: HashMap<_, _> = includes
            .media
            .expect("includes media")
            .into_iter()
            .map(|media| (media.media_key.clone(), media))
            .collect();
        let mut image_refs = Vec::new();

        for tweet in liked_tweets.into_iter() {
            let author_id = tweet.author_id.expect("author id included in response");
            let username = self.username_cache.get(&author_id);
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
                        .expect("user response")
                        .username;
                    self.username_cache.insert(author_id, username);
                    // Just inserted value above
                    &self.username_cache.get(&author_id).unwrap()
                }
            };

            let tweet_ref = TweetRef {
                username: username.to_owned(),
                created_at: tweet.created_at.expect("created at included in response"),
                id: tweet.id,
            };

            if let Some(attachments) = tweet.attachments {
                for media_key in attachments
                    .media_keys
                    .expect("attachment media keys")
                    .into_iter()
                {
                    if let Some(media) = includes_media.get(&media_key) {
                        if media.kind == MediaType::Photo {
                            let url = media.url.as_ref().expect("media url");
                            let filename = url
                                .path_segments()
                                .expect("media path segments")
                                .last()
                                .expect("no path segments");
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
