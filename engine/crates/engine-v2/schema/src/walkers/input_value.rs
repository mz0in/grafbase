use super::SchemaWalker;
use crate::{InputValueDefinitionId, TypeWalker};

pub type InputValueDefinitionWalker<'a> = SchemaWalker<'a, InputValueDefinitionId>;

impl<'a> InputValueDefinitionWalker<'a> {
    pub fn name(&self) -> &'a str {
        self.names.input_value(self.schema, self.item)
    }

    pub fn ty(&self) -> TypeWalker<'a> {
        self.walk(self.as_ref().type_id)
    }
}

impl<'a> std::fmt::Debug for InputValueDefinitionWalker<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputValue")
            .field("name", &self.name())
            .field("ty", &self.ty())
            .finish()
    }
}
