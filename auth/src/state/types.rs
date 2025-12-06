use uuid::Uuid;

use crate::models::{
    Permissions,
    user::{Role, UserState},
};

macro_rules! to_types {
    ($ty:ty, $variant: path) => {
        impl From<$ty> for Types {
            fn from(value: $ty) -> Types {
                $variant(value)
            }
        }
    };

    ($ty:ty, $variant: path, $method: path) => {
        impl From<$ty> for Types {
            fn from(value: $ty) -> Types {
                $variant($method(value))
            }
        }
    };
}

#[derive(Debug)]
pub enum Types {
    Uuid(Uuid),
    String(String),
    OptString(Option<String>),
    OptUuid(Option<Uuid>),
    UserState(UserState),
    Role(Role),
    VecAnyPermission(Vec<Permissions>),
    VecContainsPermission(Vec<Permissions>),
    VecOverlapsPermission(Vec<Permissions>),
}

to_types!(Uuid, Types::Uuid);
to_types!(Option<Uuid>, Types::OptUuid);
to_types!(String, Types::String);
to_types!(Option<String>, Types::OptString);
to_types!(UserState, Types::UserState);
to_types!(Role, Types::Role);
