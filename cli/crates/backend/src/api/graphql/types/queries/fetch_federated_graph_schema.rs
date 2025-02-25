use super::super::schema;

#[derive(cynic::QueryVariables)]
pub struct FetchFederatedGraphSchemaArguments<'a> {
    pub account: &'a str,
    pub project: &'a str,
    pub branch: &'a str,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "FetchFederatedGraphSchemaArguments")]
pub struct FetchFederatedGraphSchemaQuery {
    #[arguments(accountSlug: $account, projectSlug: $project, name: $branch)]
    pub branch: Option<Branch>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Project {
    pub branches: BranchConnection,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct BranchConnection {
    pub nodes: Vec<Branch>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Branch {
    pub name: String,
    pub schema: Option<String>,
}
