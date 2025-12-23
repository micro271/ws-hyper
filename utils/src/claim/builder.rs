use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};
use time::OffsetDateTime;

use super::Claim;




pub struct Id<T>(T);

pub struct NoId;

pub struct NoMetadata;

pub struct Metadata<M>(M);

pub struct ClaimBuilder<T, M> {
    jti: bool,
    nbf: Option<i64>,
    sub: T,
    exp: Option<i64>,
    iss: Option<String>,
    iat: bool,
    aud: Option<String>,
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
            aud: None,
            metadata: NoMetadata,
        }
    }
}

impl<T> ClaimBuilder<Id<T>, NoMetadata> {
    pub fn build(self) -> Claim<T> {
        let Self { iat ,jti, nbf, sub: Id(sub), exp, iss , aud, ..} = self;

        Claim { iss, sub, exp: exp.unwrap_or((OffsetDateTime::now_utc() + Duration::from_mins(15)).unix_timestamp()), nbf, iat: iat.then_some(OffsetDateTime::now_utc().unix_timestamp()), jti: jti.then_some(uuid::Uuid::new_v4()), aud }
    }
}

impl<T, M> ClaimBuilder<Id<T>, Metadata<M>> 
where 
    T: Serialize + DeserializeOwned,
    M: Serialize + DeserializeOwned,
{
    pub fn build(self) -> super::with_metadata::ClaimWithMetadata<T, M> {
        let Self { iat ,jti, nbf, sub: Id(sub), exp, iss , aud, metadata: Metadata(metadata)} = self;

        super::with_metadata::ClaimWithMetadata { iss, sub, exp: exp.unwrap_or((OffsetDateTime::now_utc() + Duration::from_mins(15)).unix_timestamp()), nbf, iat: iat.then_some(OffsetDateTime::now_utc().unix_timestamp()), jti: jti.then_some(uuid::Uuid::new_v4()), aud, metadata }
    }
}

impl<M> ClaimBuilder<NoId, M> {
    pub fn sub<S>(self, sub: S) -> ClaimBuilder<Id<S>, M> 
    where 
        S: Serialize + DeserializeOwned,
    {
        let Self { jti, nbf, exp, iss, iat, metadata, aud, .. } = self;

        ClaimBuilder { jti, nbf, sub: Id(sub), exp, iss, iat, metadata, aud }
    }
}

impl<I> ClaimBuilder<I, NoMetadata> {
    pub fn metadata<M>(self, meta: M) -> ClaimBuilder<I, Metadata<M>> 
    where 
        M: Serialize + DeserializeOwned,
    {
        let Self { jti, nbf, exp, iss, iat, sub, aud, .. } = self;

        ClaimBuilder { jti, nbf, sub, exp, iss, iat, metadata: Metadata(meta), aud }
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