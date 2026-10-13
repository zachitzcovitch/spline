use crate::{
    Context, Direction, Error, Field, MissingField, OptionSchema, Presence, Result, Schema, Shape,
};

/// Facts emitted by the derive after direction-specific attribute parsing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldRules {
    pub skipped: bool,
    pub defaulted: bool,
    pub omission: Omission,
    pub custom_codec: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Omission {
    #[default]
    Never,
    IfNone,
    /// The property may be omitted; its value shape is not further narrowed.
    Conditional,
}

impl Context<'_> {
    pub fn field<T: Schema>(&mut self, name: &str, rules: FieldRules) -> Result<Option<Field>> {
        if rules.skipped {
            return Ok(None);
        }
        if rules.custom_codec {
            return self.fail(Error::MissingCodec(name.into()));
        }
        if self.direction() == Direction::Serialize && rules.omission == Omission::IfNone {
            return self.fail(Error::InvalidField(format!(
                "field {name} needs typed Option information for IfNone"
            )));
        }
        let shape = self.schema::<T>()?;
        Ok(Some(self.make_field(
            name,
            shape,
            T::missing_field(),
            rules,
        )))
    }

    pub fn option_field<T: OptionSchema>(
        &mut self,
        name: &str,
        rules: FieldRules,
    ) -> Result<Option<Field>> {
        if rules.skipped {
            return Ok(None);
        }
        if rules.custom_codec {
            return self.fail(Error::MissingCodec(name.into()));
        }
        let shape =
            if self.direction() == Direction::Serialize && rules.omission == Omission::IfNone {
                self.schema::<T::Value>()?
            } else {
                self.schema::<T>()?
            };
        Ok(Some(self.make_field(
            name,
            shape,
            T::missing_field(),
            rules,
        )))
    }

    /// Explicitly describe this field's present wire values in the selected
    /// direction. Wire is a schema provider, not a replacement serializer.
    /// With conditional omission, it must describe the values actually emitted.
    /// Present-value descriptions do not establish the original type's missing
    /// behavior, so input omission requires a declared default in FieldRules.
    pub fn field_with<Wire: Schema>(
        &mut self,
        name: &str,
        rules: FieldRules,
    ) -> Result<Option<Field>> {
        if rules.skipped {
            return Ok(None);
        }
        let shape = self.schema::<Wire>()?;
        Ok(Some(self.make_field(
            name,
            shape,
            MissingField::Rejected,
            rules,
        )))
    }

    fn make_field(
        &self,
        name: &str,
        shape: Shape,
        missing: MissingField,
        rules: FieldRules,
    ) -> Field {
        let optional = match self.direction() {
            Direction::Serialize => rules.omission != Omission::Never,
            // Serde's field-level deserialize_with does not get called when
            // the field is absent. Without a default, that absence is an error.
            Direction::Deserialize => {
                rules.defaulted || (!rules.custom_codec && missing == MissingField::Accepted)
            }
        };
        Field {
            name: name.into(),
            shape,
            presence: if optional {
                Presence::Optional
            } else {
                Presence::Required
            },
            documentation: Vec::new(),
        }
    }
}
