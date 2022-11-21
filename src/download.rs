use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Local file error")]
    File(std::io::Error),
    #[error("Remote file error")]
    Remote(reqwest::Error),
}

pub async fn file(client: &reqwest::Client, url: url::Url, path: &Path) -> Result<(), Error> {
    let mut file = std::fs::File::create(path).map_err(Error::File)?;
    let bytes = &client
        .get(url)
        .send()
        .await
        .map_err(Error::Remote)?
        .bytes()
        .await
        .map_err(Error::Remote)?;
    file.write_all(bytes).map_err(Error::File)?;
    Ok(())
}
