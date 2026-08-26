use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_uuid_identifier_type {
    ($identifier_type:ident) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $identifier_type(Uuid);

        impl $identifier_type {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }

            pub(crate) fn as_bytes(self) -> [u8; 16] {
                *self.0.as_bytes()
            }
        }

        impl Default for $identifier_type {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $identifier_type {
            fn from(identifier: Uuid) -> Self {
                Self(identifier)
            }
        }

        impl From<$identifier_type> for Uuid {
            fn from(identifier: $identifier_type) -> Self {
                identifier.0
            }
        }

        impl Display for $identifier_type {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.0, formatter)
            }
        }

        impl FromStr for $identifier_type {
            type Err = uuid::Error;

            fn from_str(identifier: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(identifier).map(Self)
            }
        }
    };
}

define_uuid_identifier_type!(ProjectId);
define_uuid_identifier_type!(ChatId);
define_uuid_identifier_type!(MessageId);
define_uuid_identifier_type!(AttachmentId);
define_uuid_identifier_type!(LlmActionId);
define_uuid_identifier_type!(LlmActionStatusEventId);
