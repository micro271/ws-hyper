use std::time::Duration;

use time::OffsetDateTime;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub struct Id<T>(T);

pub struct NoId;
pub struct NoMetadata;

pub struct ClaimBuilder<T, M> {
    jti: bool,
    nbf: Option<i64>,
    sub: T,
    exp: Option<i64>,
    iss: Option<String>,
    iat: bool,
    metadata: M,
}

impl std::default::Default for ClaimBuilder<NoId, NoMetadata> {
    fn default() -> Self {
        Self {
            sub: NoId,
            jti: false,
            nbf: None,
            exp: None,
            iss: None,
            iat: false,
            metadata: NoMetadata,
        }
    }
}

impl<T, M: ClaimMetadata> ClaimBuilder<Id<T>, M> {
    pub fn build(self) -> Claim<T, M> {
        let Self { iat ,jti, nbf, sub: Id(sub), exp, iss , metadata} = self;

        Claim { iss, sub, exp: exp.unwrap_or((OffsetDateTime::now_utc() + Duration::from_mins(15)).unix_timestamp()), nbf, iat: iat.then_some(OffsetDateTime::now_utc().unix_timestamp()), jti: jti.then_some(uuid::Uuid::new_v4()), metadata }
    }
}

impl<M> ClaimBuilder<NoId, M> {
    pub fn sub<S>(self, sub: S) -> ClaimBuilder<Id<S>, M> 
    where 
        S: Serialize + DeserializeOwned,
    {
        let Self { jti, nbf, exp, iss, iat, metadata, .. } = self;

        ClaimBuilder { jti, nbf, sub: Id(sub), exp, iss, iat, metadata }
    }
}

impl<I> ClaimBuilder<I, NoMetadata> {
    pub fn metadata<M>(self, meta: M) -> ClaimBuilder<I, M> 
    where 
        M: Serialize + DeserializeOwned + ClaimMetadata,
    {
        let Self { jti, nbf, exp, iss, iat, sub, .. } = self;

        ClaimBuilder { jti, nbf, sub, exp, iss, iat, metadata: meta }
    }
}

impl<T, M> ClaimBuilder<T, M> {
    pub fn jti(mut self, jti: bool) -> Self {
        self.jti = jti;
        self
    }
    pub fn nbf(mut self, nbf: OffsetDateTime) -> Self {
        self.nbf = Some(nbf.unix_timestamp());
        self
    }
    pub fn exp(mut self, duration: Duration) -> Self {
        self.exp = Some((OffsetDateTime::now_utc() + duration).unix_timestamp());
        self
    }
    pub fn iss(mut self, iss: String) -> Self {
        self.iss = Some(iss);
        self
    }

    pub fn iat(mut self, iss: bool) -> Self {
        self.iat = iss;
        self
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claim<T, M: ClaimMetadata> {
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,

    sub: T,
    exp: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<uuid::Uuid>,

    #[serde(skip_serializing_if = "ClaimMetadata::is_empty")]
    #[serde(flatten)]
    metadata: M,
}

impl<T, M: ClaimMetadata> Claim<T, M> {
    pub fn set_iss(&mut self, iss: String)  {
        self.iss = Some(iss);
    }
    pub fn iss(&self) -> Option<&String> {
        self.iss.as_ref()
    }
    pub fn sub(&self) -> &T {
        &self.sub
    }
    pub fn exp(&self) -> i64 {
        self.exp
    }
    pub fn nbf(&self) -> Option<i64> {
        self.nbf
    }
    pub fn iat(&self) -> Option<i64> {
        self.iat
    }
    pub fn jti(&self) -> Option<uuid::Uuid> {
        self.jti
    }
    pub fn metadata(&self) -> &M {
        self.metadata.meta()
    }
}

pub trait ClaimMetadata: Serialize {
    fn is_empty(&self) -> bool;

    fn meta(&self) -> &Self {
        self
    }
}