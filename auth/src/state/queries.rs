use super::{Table, Types};
use crate::models::Permissions;
use sqlx::{Postgres, postgres::PgArguments, query::Query};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    marker::PhantomData,
};

#[macro_export]
macro_rules! bind {
    ($q:expr, $type:expr) => {
        match $type {
            Types::Uuid(uuid) => $q.bind(uuid),
            Types::String(string) => $q.bind(string),
            Types::OptString(vec) => $q.bind(vec),
            Types::UserState(state) => $q.bind(state),
            Types::Role(role) => $q.bind(role),
            Types::OptUuid(uuid) => $q.bind(uuid),
            Types::VecAnyPermission(vec) => $q.bind(vec),
            Types::VecContainsPermission(vec) => $q.bind(vec),
            Types::VecOverlapsPermission(vec) => $q.bind(vec),
        }
    };
}

pub use bind;

type Where<'a> = HashMap<&'a str, Types>;

pub trait QuerySelect {
    fn query() -> String;
}

impl<T> QuerySelect for T
where
    T: super::Table,
{
    fn query() -> String {
        format!("SELECT * FROM {}", T::name())
    }
}

pub struct QueryOwn<'a, T> {
    wh: Option<Where<'a>>,
    _priv: PhantomData<T>,
    query: String,
    group_by: HashSet<&'a str>,
}

impl<'a, T> QueryOwn<'a, T>
where
    T: QuerySelect,
{
    pub fn builder() -> Self {
        Self {
            wh: None,
            _priv: PhantomData,
            query: String::new(),
            group_by: HashSet::new(),
        }
    }

    pub fn group_by(mut self, col: &'a str) -> Self {
        self.group_by.insert(col);

        self
    }

    pub fn wh<U>(mut self, index: &'a str, value: U) -> Self
    where
        U: Into<Types>,
    {
        if self.wh.is_none() {
            self.wh = Some(HashMap::from([(index, value.into())]));
        } else {
            self.wh.as_mut().unwrap().insert(index, value.into());
        }

        self
    }

    pub fn wh_vec_any(mut self, column: &'a str, values: Vec<Permissions>) -> Self {
        if self.wh.is_none() {
            self.wh = Some(HashMap::from([(column, Types::VecAnyPermission(values))]));
        } else {
            self.wh
                .as_mut()
                .unwrap()
                .insert(column, Types::VecAnyPermission(values));
        }

        self
    }

    pub fn build(&'a mut self) -> Query<'a, Postgres, PgArguments> {
        self.query = T::query();
        let mut aux = Vec::new();
        if let Some(wheres) = self.wh.take() {
            let mut first = true;
            let mut n = 1;
            for (key, value) in wheres {
                if first {
                    first = false;
                    self.query.push_str(" WHERE");
                } else {
                    self.query.push_str(" AND");
                }
                if let Types::VecAnyPermission(_) = &value {
                    _ = write!(self.query, " {key} = ANY(${n})");
                } else {
                    _ = write!(self.query, " {key} = ${n}");
                }
                aux.push(value);
                n += 1;
            }
        }

        if !self.group_by.is_empty() {
            let list = self.group_by.drain().collect::<Vec<_>>().join(",");
            self.query.push_str(&format!(" GROUP BY {list}"));
        }

        let mut query = sqlx::query(&self.query);
        for t in aux {
            query = bind!(query, t);
        }
        query
    }
}

pub struct InsertOwn<T> {
    query: String,
    item: Option<T>,
}

pub trait Insert<T> {
    fn insert(item: T) -> Self;
    fn query(&mut self) -> Query<'_, Postgres, PgArguments>;
}

impl<T> Insert<T> for InsertOwn<T>
where
    T: Table,
{
    fn insert(item: T) -> Self {
        let columns = T::columns();
        Self {
            query: format!(
                "INSERT INTO {} ({}) VALUES ({})",
                T::name(),
                columns.join(","),
                (1..=columns.len())
                    .map(|x| format!("${x}"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            item: Some(item),
        }
    }

    fn query(&mut self) -> Query<'_, Postgres, PgArguments> {
        let item = self.item.take();

        item.unwrap()
            .values()
            .into_iter()
            .fold(sqlx::query(&self.query), |acc, item| bind!(acc, item))
    }
}

impl<T> Insert<Vec<T>> for InsertOwn<Vec<T>> {
    fn insert(_item: Vec<T>) -> Self {
        todo!()
    }

    fn query(&mut self) -> Query<'_, Postgres, PgArguments> {
        todo!()
    }
}

pub struct UpdateOwn<'a, T> {
    pub query: String,
    pub wh: Where<'a>,
    pub items: Vec<Types>,
    _priv: PhantomData<T>,
}

impl<'a, T> UpdateOwn<'a, T>
where
    T: Table,
{
    pub fn new() -> Self {
        Self {
            query: String::new(),
            wh: HashMap::new(),
            items: Vec::new(),
            _priv: PhantomData,
        }
    }

    pub fn from<U>(mut self, items: U) -> Self
    where
        U: Into<HashMap<&'static str, Types>>,
    {
        self.query = format!("UPDATE {} SET", T::name());
        let mut count = 1;
        for (k, v) in <U as Into<HashMap<&'static str, Types>>>::into(items) {
            self.items.push(v);
            _ = write!(
                self.query,
                "{} {k} = ${count}",
                if count > 1 { "," } else { "" }
            );

            count += 1;
        }
        self
    }

    pub fn wh<U>(mut self, index: &'a str, value: U) -> Self
    where
        U: Into<Types>,
    {
        self.wh.insert(index, value.into());
        self
    }

    pub fn query(&mut self) -> Result<Query<'_, Postgres, PgArguments>, UpdateOwnErr> {
        if self.items.is_empty() {
            return Err(UpdateOwnErr);
        }
        let len = self.items.len();
        let count = len + 1;
        for (k, v) in std::mem::take(&mut self.wh) {
            _ = write!(
                self.query,
                "{} {k} = ${count}",
                if count == len + 1 { " WHERE" } else { "," }
            );

            self.items.push(v);
        }

        Ok(std::mem::take(&mut self.items)
            .into_iter()
            .fold(sqlx::query(&self.query), |x, y| bind!(x, y)))
    }
}

#[derive(Debug)]
pub struct UpdateOwnErr;

impl std::fmt::Display for UpdateOwnErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Values not defined")
    }
}

impl std::error::Error for UpdateOwnErr {}
