use crate::model::file;
use axum::body::Body;
use chrono::Utc;
use futures_util::StreamExt;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, Set};
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("file exceeds configured size limit")]
    TooLarge,
    #[error(transparent)]
    Storage(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("upload stream failed: {0}")]
    Stream(String),
}

pub fn path_for(upload_dir: &Path, id: &str) -> Option<PathBuf> {
    Uuid::parse_str(id)
        .ok()
        .map(|uuid| upload_dir.join(uuid.to_string()))
}

pub async fn save(
    db: &DatabaseConnection,
    upload_dir: &Path,
    owner_id: i32,
    filename: &str,
    content_type: &str,
    max_bytes: u64,
    body: Body,
) -> Result<file::Model, UploadError> {
    fs::create_dir_all(upload_dir).await?;
    let id = Uuid::new_v4().to_string();
    let path = upload_dir.join(&id);
    let temporary_path = upload_dir.join(format!("{id}.part"));
    let result = async {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .await?;
        let mut stream = body.into_data_stream();
        let mut size = 0_u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| UploadError::Stream(error.to_string()))?;
            size = size
                .checked_add(u64::try_from(chunk.len()).map_err(|_| UploadError::TooLarge)?)
                .ok_or(UploadError::TooLarge)?;
            if size > max_bytes {
                return Err(UploadError::TooLarge);
            }
            output.write_all(&chunk).await?;
        }
        output.flush().await?;
        drop(output);
        fs::rename(&temporary_path, &path).await?;
        let model = file::ActiveModel {
            id: Set(id),
            owner_id: Set(owner_id),
            filename: Set(filename.to_string()),
            content_type: Set(content_type.to_string()),
            size_bytes: Set(i64::try_from(size).map_err(|_| UploadError::TooLarge)?),
            created_at: Set(Utc::now()),
        }
        .insert(db)
        .await?;
        Ok(model)
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path).await;
        let _ = fs::remove_file(&path).await;
    }
    result
}

pub async fn find(db: &DatabaseConnection, id: &str) -> Result<Option<file::Model>, DbErr> {
    file::Entity::find_by_id(id).one(db).await
}

pub async fn delete(db: &DatabaseConnection, upload_dir: &Path, id: &str) -> anyhow::Result<()> {
    let Some(path) = path_for(upload_dir, id) else {
        anyhow::bail!("invalid file id");
    };
    let tombstone = path.with_extension("deleting");
    let moved = match fs::rename(&path, &tombstone).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if let Err(error) = file::Entity::delete_by_id(id).exec(db).await {
        if moved {
            fs::rename(&tombstone, &path).await?;
        }
        return Err(error.into());
    }
    if moved && let Err(error) = fs::remove_file(&tombstone).await {
        tracing::warn!(file_id = id, %error, "failed to remove deleted file bytes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_path_rejects_traversal() {
        assert!(path_for(Path::new("uploads"), "../secret").is_none());
        assert!(path_for(Path::new("uploads"), "not-an-id").is_none());
        let id = Uuid::new_v4().to_string();
        assert_eq!(
            path_for(Path::new("uploads"), &id),
            Some(PathBuf::from("uploads").join(id))
        );
    }
}
