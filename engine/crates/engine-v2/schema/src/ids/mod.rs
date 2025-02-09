/// Isolating ids from the rest to prevent misuse of the NonZeroU32.
/// They can only be created by From<usize>
use crate::{
    sources::federation::{DataSource as FederationDataSource, Subgraph},
    CacheConfig, Definition, Directive, Enum, EnumValue, Field, Header, InputObject, InputValueDefinition, Interface,
    Object, Resolver, Scalar, Schema, Type, Union,
};
use url::Url;

mod range;
pub use range::*;

/// Reserving the 4 upper bits for some fun with bit packing. It still leaves 268 million possible values.
/// And it's way easier to increase that limit if needed that to reserve some bits later!
/// Currently, we use the two upper bits of the FieldId for the ResponseEdge in the engine.
pub(crate) const MAX_ID: usize = (1 << 29) - 1;

macro_rules! id_newtypes {
    ($($ty:ident.$field:ident[$name:ident] => $out:ty | unless $msg:literal,)*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
            pub struct $name(std::num::NonZeroU32);

            impl std::ops::Index<$name> for $ty {
                type Output = $out;

                fn index(&self, index: $name) -> &$out {
                    &self.$field[(index.0.get() - 1) as usize]
                }
            }

            impl std::ops::IndexMut<$name> for $ty {
                fn index_mut(&mut self, index: $name) -> &mut $out {
                    &mut self.$field[(index.0.get() - 1) as usize]
                }
            }

            impl std::ops::Index<crate::ids::IdRange<$name>> for $ty {
                type Output = [$out];

                fn index(&self, range: crate::ids::IdRange<$name>) -> &Self::Output {
                    let start = usize::from(range.start);
                    let end = usize::from(range.end);
                    &self.$field[start..end]
                }
            }

            impl std::ops::IndexMut<crate::ids::IdRange<$name>> for $ty {
                fn index_mut(&mut self, range: crate::ids::IdRange<$name>) -> &mut Self::Output {
                    let start = usize::from(range.start);
                    let end = usize::from(range.end);
                    &mut self.$field[start..end]
                }
            }

            impl From<usize> for $name {
                fn from(index: usize) -> Self {
                    assert!(index <= MAX_ID, "{}", $msg);
                    Self(std::num::NonZeroU32::new((index + 1) as u32).unwrap())
                }
            }

            impl From<$name> for usize {
                fn from(id: $name) -> Self {
                    (id.0.get() - 1) as usize
                }
            }

            impl From<$name> for u32 {
                fn from(id: $name) -> Self {
                    (id.0.get() - 1)
                }
            }
        )*
    }
}

id_newtypes! {
    Schema.definitions[DefinitionId] => Definition | unless "Too many definitions",
    Schema.directives[DirectiveId] => Directive | unless "Too many directives",
    Schema.enum_values[EnumValueId] => EnumValue | unless "Too many enum values",
    Schema.enums[EnumId] => Enum | unless "Too many enums",
    Schema.fields[FieldId] => Field | unless "Too many fields",
    Schema.headers[HeaderId] => Header | unless "Too many headers",
    Schema.input_objects[InputObjectId] => InputObject | unless "Too many input objects",
    Schema.input_value_definitions[InputValueDefinitionId] => InputValueDefinition | unless "Too many input value definitions",
    Schema.interfaces[InterfaceId] => Interface | unless "Too many interfaces",
    Schema.objects[ObjectId] => Object | unless "Too many objects",
    Schema.resolvers[ResolverId] => Resolver | unless "Too many resolvers",
    Schema.scalars[ScalarId] => Scalar | unless "Too many scalars",
    Schema.types[TypeId] => Type | unless "Too many types",
    Schema.unions[UnionId] => Union | unless "Too many unions",
    Schema.urls[UrlId] => Url | unless "Too many urls",
    Schema.strings[StringId] => String | unless "Too many strings",
    FederationDataSource.subgraphs[SubgraphId] => Subgraph | unless "Too many subgraphs",
    Schema.cache_configs[CacheConfigId] => CacheConfig | unless "Too many cache configs",
}
