use serde::{
    Serialize, Deserialize,
    ser::Serializer,
    de::{self, Deserializer, Unexpected, Visitor},
};
use std::hash::Hash;

/// An unsigned 53-bit integer.
///
/// This is a minimal implementation that is intended soley for serialization
/// and comparison. In the current landscape, a 53-bit integer is a better
/// choice for a portable, double-worded integer than a 64-bit integer because
/// it is the largest sized integer possible in Javascript without losing
/// precision.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Hash)]
pub struct u53(u64);

impl From<u64> for u53 {
    fn from(v: u64) -> Self {
        u53(v & 0x1fffffffffffffu64)
    }
}

impl From<u32> for u53 {
    fn from(v: u32) -> Self {
        u53(v as u64)
    }
}

impl From<u16> for u53 {
    fn from(v: u16) -> Self {
        u53(v as u64)
    }
}

impl From<u8> for u53 {
    fn from(v: u8) -> Self {
        u53(v as u64)
    }
}

impl From<i64> for u53 {
    fn from(v: i64) -> Self {
        u53((v as u64).into())
    }
}

impl From<i32> for u53 {
    fn from(v: i32) -> Self {
        u53((v as u32).into())
    }
}

impl From<i16> for u53 {
    fn from(v: i16) -> Self {
        u53((v as u16).into())
    }
}

impl From<i8> for u53 {
    fn from(v: i8) -> Self {
        u53((v as u8).into())
    }
}

impl From<u53> for u64 {
    fn from(v: u53) -> u64 {
        v.0
    }
}

impl Serialize for u53 {
    fn serialize<S>(
        &self,
        serializer: S
    ) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_u64(self.clone().into())
    }
}

impl<'de> Deserialize<'de> for u53 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        struct U53Visitor;
        impl<'de> Visitor<'de> for U53Visitor {
            type Value = u53;
            fn visit_u64<E>(self, value: u64) -> Result<u53, E>
            where E: de::Error {
                if (value & 0xffe0000000000000u64) == 0 {
                    Ok(value.into())
                } else {
                    Err(de::Error::invalid_value(
                        Unexpected::Unsigned(value),
                        &self
                    ))
                }
            }
            fn visit_u32<E>(self, value: u32) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn visit_u16<E>(self, value: u16) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn visit_u8<E>(self, value: u8) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn visit_i64<E>(self, value: i64) -> Result<u53, E>
            where E: de::Error {
                if value >= 0
                    && ((value as u64) & 0xffe0000000000000u64) == 0 {
                    Ok(value.into())
                } else {
                    Err(de::Error::invalid_value(
                        Unexpected::Signed(value),
                        &self
                    ))
                }
            }
            fn visit_i32<E>(self, value: i32) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn visit_i16<E>(self, value: i16) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn visit_i8<E>(self, value: i8) -> Result<u53, E>
            where E: de::Error {
                Ok(value.into())
            }
            fn expecting(
                &self,
                f: &mut std::fmt::Formatter<'_>
            ) -> Result<(), std::fmt::Error> {
                write!(f, "u53")
            }
        }
        deserializer.deserialize_u64(U53Visitor)
    }
}

impl Eq for u53 {}

impl PartialEq for u53 {
    fn eq(&self, other: &u53) -> bool {
        self.0 == other.0
    }
}

impl std::fmt::Display for u53 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
