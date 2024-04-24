use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone)]
pub struct LimitedVec<T, const N: usize>(pub Vec<T>);

impl<'de, T, const N: usize> Deserialize<'de> for LimitedVec<T, N>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Vec<T> as Deserialize>::deserialize(deserializer).map(|vec| Self(vec))
    }
}

impl<T, const N: usize> Serialize for LimitedVec<T, N>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}
