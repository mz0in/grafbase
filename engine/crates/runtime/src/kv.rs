use std::{sync::Arc, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error("Kv error: {0}")]
    Kv(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type KvResult<T> = Result<T, KvError>;

#[derive(Clone)]
pub struct KvStore(Arc<dyn KvStoreInner>);

impl KvStore {
    pub fn new(inner: impl KvStoreInner + 'static) -> Self {
        Self(Arc::new(inner))
    }

    pub async fn get_json_or_null<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        cache_ttl: Option<Duration>,
    ) -> KvResult<Option<T>> {
        let bytes = self.get(name, cache_ttl).await?;
        Ok(bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        cache_ttl: Option<Duration>,
    ) -> KvResult<Option<T>> {
        let bytes = self.get(name, cache_ttl).await?;
        bytes
            .map(|bytes| serde_json::from_slice(&bytes))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn put_json<T: serde::Serialize>(
        &self,
        name: &str,
        value: &T,
        expiration_ttl: Option<Duration>,
    ) -> KvResult<()> {
        let bytes = serde_json::to_vec(value)?;
        self.put(name, bytes, expiration_ttl).await
    }
}

impl std::ops::Deref for KvStore {
    type Target = dyn KvStoreInner;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[async_trait::async_trait]
pub trait KvStoreInner: Send + Sync {
    async fn get(&self, name: &str, cache_ttl: Option<Duration>) -> KvResult<Option<Vec<u8>>>;
    async fn put(&self, name: &str, bytes: Vec<u8>, expiration_ttl: Option<Duration>) -> KvResult<()>;
}
