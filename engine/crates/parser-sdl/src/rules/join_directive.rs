use std::collections::{BTreeSet, HashMap};

use engine::registry::{field_set, resolvers::join::JoinResolver, FieldSet};
use engine_parser::{
    parse_field,
    types::{ConstDirective, Field},
    Positioned,
};
use engine_value::{Name, Value};
use serde::de::Error;

use super::{directive::Directive, visitor::VisitorContext};
use crate::directive_de::parse_directive;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JoinDirective {
    pub select: FieldSelection,
}

#[derive(Debug)]
pub struct FieldSelection {
    selections: Vec<Selection>,
    required_fields: Vec<String>,
}

#[derive(Debug)]
struct Selection {
    field_name: String,
    arguments: Vec<(Name, Value)>,
}

impl Directive for JoinDirective {
    fn definition() -> String {
        "
        directive @join(select: FieldSelection!) on FIELD_DEFINITION
        "
        .into()
    }
}

impl JoinDirective {
    pub fn from_directives(
        directives: &[Positioned<ConstDirective>],
        ctx: &mut VisitorContext<'_>,
    ) -> Option<JoinDirective> {
        let directive = directives.iter().find(|directive| directive.name.node == "join")?;

        match parse_directive::<Self>(directive, &HashMap::new()) {
            Ok(directive) => Some(directive),
            Err(error) => {
                ctx.append_errors(vec![error]);
                None
            }
        }
    }
}

impl FieldSelection {
    pub fn required_fieldset(&self) -> Option<FieldSet> {
        if self.required_fields.is_empty() {
            return None;
        }

        Some(FieldSet::new(self.required_fields.iter().map(|field| {
            field_set::Selection {
                field: field.clone(),
                selections: vec![],
            }
        })))
    }

    pub fn to_join_resolver(&self) -> JoinResolver {
        JoinResolver::new(
            self.selections
                .iter()
                .map(|selection| (selection.field_name.clone(), selection.arguments.clone())),
        )
    }
}

impl<'de> serde::Deserialize<'de> for FieldSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let select = String::deserialize(deserializer)?;

        let field = parse_field(select).map_err(|error| D::Error::custom(format!("Could not parse join: {error}")))?;

        validate_field(&field).map_err(D::Error::custom)?;

        let required_fields = field
            .node
            .arguments
            .iter()
            .flat_map(|(_, value)| value.variables_used())
            .map(|variable| variable.to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        Ok(FieldSelection {
            selections: SelectionIter {
                field: Some(field.node),
            }
            .collect(),
            required_fields,
        })
    }
}

fn validate_field(mut field: &Positioned<Field>) -> Result<(), String> {
    loop {
        match field.node.selection_set.node.items.as_slice() {
            [] => return Ok(()),
            [item] => match &item.node {
                engine_parser::types::Selection::Field(inner_field) => {
                    field = inner_field;
                    continue;
                }
                _ => return Err("joins can't make use of spreads".into()),
            },
            _ => return Err("joins can contain at most a single field per selection set".into()),
        }
    }
}

struct SelectionIter {
    field: Option<Field>,
}

impl Iterator for SelectionIter {
    type Item = Selection;

    fn next(&mut self) -> Option<Self::Item> {
        let field = std::mem::take(&mut self.field)?;

        let next = Selection {
            field_name: field.name.to_string(),
            arguments: field
                .arguments
                .into_iter()
                .map(|(name, value)| (name.node, value.node))
                .collect(),
        };

        self.field = field
            .selection_set
            .node
            .items
            .into_iter()
            .next()
            .and_then(|selection| match selection.node {
                engine_parser::types::Selection::Field(inner_field) => Some(inner_field.node),
                _ => None,
            });

        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_json_snapshot};
    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::tests::assert_validation_error;

    #[test]
    fn join_happy_path() {
        let schema = r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): String! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id)")
            }
        "#;

        let registry = crate::to_parse_result_with_variables(schema, &HashMap::new())
            .unwrap()
            .registry;

        let resolver = &registry.types["User"].fields().as_ref().unwrap()["nickname"].resolver;

        assert_json_snapshot!(resolver, @r###"
        {
          "J": {
            "fields": [
              [
                "blah",
                [
                  [
                    "id",
                    {
                      "$var": "id"
                    }
                  ]
                ]
              ]
            ]
          }
        }
        "###);
    }

    #[test]
    fn nested_join_with_arguments_and_such() {
        let schema = r#"
            extend type Query {
                greetings(name: String!): Greetings @resolver(name: "greetings")
                user: User! @resolver(name: "user")
            }

            type User {
                id: ID!
                name: String!
                greeting: String! @join(
                    select: "greetings(name: $name) { forTimeOfDay(id: $id, timeOfDay: \"morning\") }"
                )
            }

            type Greetings {
                forTimeOfDay(id: String!, timeOfDay: String!): String! @resolver(name: "timeOfDayGreeting")
            }
        "#;

        let registry = crate::to_parse_result_with_variables(schema, &HashMap::new())
            .unwrap()
            .registry;

        let resolver = &registry.types["User"].fields().as_ref().unwrap()["greeting"].resolver;

        assert_json_snapshot!(resolver, @r###"
        {
          "J": {
            "fields": [
              [
                "greetings",
                [
                  [
                    "name",
                    {
                      "$var": "name"
                    }
                  ]
                ]
              ],
              [
                "forTimeOfDay",
                [
                  [
                    "id",
                    {
                      "$var": "id"
                    }
                  ],
                  [
                    "timeOfDay",
                    "morning"
                  ]
                ]
              ]
            ]
          }
        }
        "###);
    }

    #[test]
    fn join_with_missing_required_argument() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String!): String! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with Query.blah, but does not provide the non-nullable argument name"
        );
    }

    #[test]
    fn join_with_list_in_path() {
        assert_validation_error!(
            r#"
            extend type Query {
                greetings(name: String!): [Greetings] @resolver(name: "greetings")
                user: User! @resolver(name: "user")
            }

            type User {
                id: ID!
                name: String!
                greeting: String! @join(
                    select: "greetings(name: $name) { forTimeOfDay(id: $id, timeOfDay: \"morning\") }"
                )
            }

            type Greetings {
                forTimeOfDay(id: String!, timeOfDay: String!): String! @resolver(name: "timeOfDayGreeting")
            }
            "#,
            "The join on User.greeting passes through Query.greetings, which is a list.  This is not supported"
        );
    }

    #[test]
    fn join_with_nested_multiple_fields() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String!): String! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id) { foo bar }")
            }
            "#,
            "joins can contain at most a single field per selection set"
        );
    }

    #[test]
    fn join_on_missing_field() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with a field named blah, which doesn't exist on the Query type"
        );
    }

    #[test]
    fn join_through_scalar() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                foo: String! @resolver(name: "foo")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "foo { bar }")
            }
            "#,
            "The join on User.nickname tries to select children of String, but String is not a composite type"
        );
    }

    #[test]
    fn join_with_a_spread() {
        assert_validation_error!(
            r#"
            extend type Query {
                greetings(name: String!): Greetings @resolver(name: "greetings")
                user: User! @resolver(name: "user")
            }

            type User {
                id: ID!
                name: String!
                greeting: String! @join(
                    select: """
                        greetings(name: $name) {
                            ... on Greetings {
                                forTimeOfDay(id: $id, timeOfDay: "morning")
                            }
                        }
                    """
                )
            }

            type Greetings {
                forTimeOfDay(id: String!, timeOfDay: String!): String! @resolver(name: "timeOfDayGreeting")
            }
            "#,
            "joins can't make use of spreads"
        );
    }

    #[test]
    fn join_with_return_type_mismatch() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!): Int @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with Query.blah, but those fields do not have compatible types"
        );
    }

    #[test]
    fn join_with_variable_that_doesnt_exist() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!): String! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $whatever)")
            }
            "#,
            "User.nickname declares that it requires the field whatever on User but that field doesn't exist"
        );
    }

    #[test]
    fn acceptable_return_type_nullability_mismatch() {
        let schema = r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): String! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String @join(select: "blah(id: $id)")
            }
            "#;

        let registry = crate::to_parse_result_with_variables(schema, &HashMap::new())
            .unwrap()
            .registry;

        let resolver = &registry.types["User"].fields().as_ref().unwrap()["nickname"].resolver;

        assert_json_snapshot!(resolver);
    }

    #[test]
    fn return_type_nullability_mismatch() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): String @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String! @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with Query.blah, but those fields do not have compatible types"
        );
    }

    #[test]
    fn return_type_lists_success() {
        let schema = r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): [String!]! @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: [String] @join(select: "blah(id: $id)")
            }
            "#;

        let registry = crate::to_parse_result_with_variables(schema, &HashMap::new())
            .unwrap()
            .registry;

        let resolver = &registry.types["User"].fields().as_ref().unwrap()["nickname"].resolver;

        assert_json_snapshot!(resolver);
    }

    #[test]
    fn return_list_nullability_mismatch() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): [String] @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: [String]! @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with Query.blah, but those fields do not have compatible types"
        );
    }

    #[test]
    fn return_list_mismatch() {
        assert_validation_error!(
            r#"
            extend schema @federation(version: "2.3")

            extend type Query {
                blah(id: String!, name: String): [String] @resolver(name: "blah")
            }

            type User @key(fields: "id", resolvable: false) {
                id: ID!
                nickname: String @join(select: "blah(id: $id)")
            }
            "#,
            "User.nickname is trying to join with Query.blah, but those fields do not have compatible types"
        );
    }

    #[test]
    fn join_directive_deser() {
        let directive = JoinDirective::deserialize(json!({"select": "findUser(name: $name, filters: {eq: $filters})"}));

        assert_debug_snapshot!(directive, @r###"
        Ok(
            JoinDirective {
                select: FieldSelection {
                    selections: [
                        Selection {
                            field_name: "findUser",
                            arguments: [
                                (
                                    Name(
                                        "name",
                                    ),
                                    Variable(
                                        Name(
                                            "name",
                                        ),
                                    ),
                                ),
                                (
                                    Name(
                                        "filters",
                                    ),
                                    Object(
                                        {
                                            Name(
                                                "eq",
                                            ): Variable(
                                                Name(
                                                    "filters",
                                                ),
                                            ),
                                        },
                                    ),
                                ),
                            ],
                        },
                    ],
                    required_fields: [
                        "filters",
                        "name",
                    ],
                },
            },
        )
        "###);
    }
}
